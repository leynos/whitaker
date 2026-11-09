//! Macro-free score parity between this lint and the pinned Clippy component.
//!
//! RFC 0010 §7.4 states that Whitaker reuses Clippy's arithmetic and changes
//! only the exclusion predicate, so pinned Clippy is the oracle. A function with
//! no macro-generated nodes must score exactly as Clippy scores it. This test
//! derives that expectation from two independent runs of a macro-free fixture:
//!
//! - `cargo clippy --lib --message-format=json -- -W clippy::cognitive_complexity` with
//!   `clippy.toml` setting `cognitive-complexity-threshold = 0`;
//! - `cargo dylint --all -- --message-format json` with this crate's library on
//!   `DYLINT_LIBRARY_PATH` and `dylint.toml` setting `max_complexity = 0`.
//!
//! Both runs report one diagnostic per scored body. Scores are parsed from the
//! rendered message and anchored on the primary span's start line; the vectors
//! are compared per anchor line and against a handwritten table pinning RFC
//! 0010 Table 7 and `ExecPlan` V4.
//!
//! # Selection differences
//!
//! - Clippy rejects a body whose span is `from_expansion()`; Whitaker tests the same predicate and
//!   also rejects a generated anchor. For macro-free source both selections coincide, including the
//!   desugared `async fn` wrapper.
//! - Clippy anchors a closure by scanning for `|`; Whitaker uses `fn_decl_span`. Both land on the
//!   closure header line, so comparison is per line.
//! - Clippy traverses with `for_each_expr_without_closures`, so it does not descend into closure
//!   bodies. Whitaker's default-nested-filter `intravisit::Visitor` reproduces that boundary; those
//!   names are the pinned evidence.
//! - Clippy avoids double-reporting an `async fn` wrapper by failing to find a `|` in its span;
//!   Whitaker skips coroutine wrappers explicitly. Both emit exactly one diagnostic, which this
//!   test pins.
//!
//! # Blind spots
//!
//! The fixture cannot compare macro-generated bodies or the authored contents
//! of an `async` block, whose wrapper both tools skip. The UI fixtures cover
//! both cases.

#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]
#![cfg(feature = "dylint-driver")]

mod support;

use anyhow::Context as _;
use serde_json::Value;
use tempfile::TempDir;

use self::support::{
    CLIPPY_CODE,
    LINT_CODE,
    RawRun,
    build_and_stage_library,
    compiler_messages,
    ensure_tool,
    fixture_line,
    lint_scores,
    private_target_dir,
    run_clippy,
    run_whitaker,
    utf8_temp_path,
    workspace_root,
    write_fixture,
};

/// Expected `(anchor line, score)` pairs, sorted as both tools emit them.
const EXPECTED_SCORES: &[(u32, u64)] = &[
    (3, 1),  // baseline
    (5, 2),  // one_if
    (7, 3),  // two_ifs
    (9, 2),  // multi_arm_match
    (11, 1), // single_arm_match
    (13, 3), // guarded_match
    (15, 1), // adjacent_returns
    (17, 2), // separated_returns
    (19, 1), // separated_result_returns
    (21, 1), // returns_result
    (23, 2), // source_for
    (25, 2), // source_while
    (27, 3), // source_for_if
    (29, 1), // try_result
    (31, 2), // try_option
    (33, 1), // closure_body, enclosing function
    (33, 2), // closure_body, closure
    (35, 2), // async_if
    (37, 2), // async_await
];

/// Assert each run's exit status: Clippy warns, Whitaker denies.
fn assert_run_status(clippy_run: &RawRun, whitaker_run: &RawRun) {
    assert!(
        clippy_run.is_success,
        "cargo clippy should succeed because `-W` only warns\nstderr:\n{}",
        clippy_run.stderr
    );
    assert!(
        !whitaker_run.is_success,
        "cargo dylint should fail because `-D warnings` denies the lint findings"
    );
}

/// Assert every coded error-level message names this lint.
///
/// Rustc also emits an uncoded `aborting due to N previous errors` summary at
/// error level, so only a foreign code is evidence of an unrelated failure.
fn assert_only_lint_errors(whitaker_run: &RawRun) {
    for message in compiler_messages(whitaker_run) {
        let Some(diagnostic) = message.get("message") else {
            continue;
        };
        if diagnostic.get("level").and_then(Value::as_str) != Some("error") {
            continue;
        }
        let Some(code) = diagnostic
            .get("code")
            .and_then(|entry| entry.get("code"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        assert_eq!(
            code,
            LINT_CODE,
            "fixture compilation produced an unrelated error: {}\nstderr:\n{}",
            diagnostic
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            whitaker_run.stderr
        );
    }
}

/// Assert both runs scored at least the fixture floor of 15 bodies.
fn assert_minimum_counts(clippy_scores: &[(u32, u64)], whitaker_scores: &[(u32, u64)]) {
    assert!(
        clippy_scores.len() >= 15,
        "Clippy produced {} scored diagnostics, expected at least 15",
        clippy_scores.len()
    );
    assert!(
        whitaker_scores.len() >= 15,
        "Whitaker produced {} scored diagnostics, expected at least 15",
        whitaker_scores.len()
    );
}

/// Assert the pinned table first, then per-anchor parity with Clippy.
fn assert_score_parity(clippy_scores: &[(u32, u64)], whitaker_scores: &[(u32, u64)]) {
    assert_eq!(
        clippy_scores, EXPECTED_SCORES,
        "Clippy scores must match the handwritten table"
    );
    assert_eq!(
        whitaker_scores, clippy_scores,
        "scores must agree per anchor line"
    );
}

/// Assert exactly one diagnostic is anchored on the `async fn` line.
fn assert_single_async_diagnostic(scores: &[(u32, u64)], async_line: u32) {
    let async_count = scores
        .iter()
        .filter(|(line, _)| *line == async_line)
        .count();
    assert_eq!(
        async_count, 1,
        "the `async fn` on line {async_line} must produce exactly one diagnostic"
    );
}

/// Assert macro-free scores match the pinned Clippy oracle.
#[test]
fn macro_free_scores_match_pinned_clippy() -> anyhow::Result<()> {
    ensure_tool("cargo clippy -V", &["clippy", "-V"])?;
    ensure_tool("cargo dylint --version", &["dylint", "--version"])?;
    let workspace = workspace_root()?;
    let library_target = private_target_dir(&workspace, "parity-lint-")?;
    let clippy_target = private_target_dir(&workspace, "parity-clippy-")?;
    let whitaker_target = private_target_dir(&workspace, "parity-whitaker-")?;

    let library_dir = build_and_stage_library(&workspace, &utf8_temp_path(&library_target)?)?;
    let fixture = TempDir::new().context("failed to create the fixture directory")?;
    let fixture_root = utf8_temp_path(&fixture)?;
    write_fixture(&fixture_root)?;

    let clippy_run = run_clippy(fixture_root.as_std_path(), &utf8_temp_path(&clippy_target)?)?;
    let whitaker_run = run_whitaker(
        fixture_root.as_std_path(),
        &utf8_temp_path(&whitaker_target)?,
        &library_dir,
    )?;
    let clippy_scores = lint_scores(&clippy_run, CLIPPY_CODE)?;
    let whitaker_scores = lint_scores(&whitaker_run, LINT_CODE)?;

    assert_run_status(&clippy_run, &whitaker_run);
    assert_only_lint_errors(&whitaker_run);
    assert_minimum_counts(&clippy_scores, &whitaker_scores);
    assert_score_parity(&clippy_scores, &whitaker_scores);
    assert_single_async_diagnostic(&whitaker_scores, fixture_line("pub async fn async_if")?);
    Ok(())
}
