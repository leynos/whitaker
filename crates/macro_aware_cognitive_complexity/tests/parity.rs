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

use std::{
    env,
    io::Cursor,
    path::Path,
    process::{Command, Output},
};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use cargo_metadata::Message;
use serde_json::Value;
use tempfile::TempDir;

const LINT_PACKAGE: &str = "macro_aware_cognitive_complexity";
const CLIPPY_CODE: &str = "clippy::cognitive_complexity";
const LINT_CODE: &str = "macro_aware_cognitive_complexity";
const SCORE_MARKER: &str = "cognitive complexity of ";

const FIXTURE_MANIFEST: &str = r#"[package]
name = "parity_fixture"
version = "0.1.0"
edition = "2024"

[workspace]

[lib]
path = "src/lib.rs"
"#;
const FIXTURE_CLIPPY_CONFIG: &str = "cognitive-complexity-threshold = 0\n";
const FIXTURE_DYLINT_CONFIG: &str = r"[macro_aware_cognitive_complexity]
max_complexity = 0
";

/// Macro-free fixture source, each item preceded by a comment naming its construct.
const FIXTURE_SOURCE: &str = r"//! Macro-free fixture pinning cognitive complexity scores for parity checks.
// baseline: no authored branching scores 1.
pub fn baseline() {}
// one_if: one authored `if` adds one to the baseline.
pub fn one_if(flag: bool) { if flag {} }
// two_ifs: two authored `if` expressions add two.
pub fn two_ifs(flag: bool) { if flag {} if !flag {} }
// multi_arm_match: a three-arm match is one branch.
pub fn multi_arm_match(value: u8) { match value { 0 => {}, 1 => {}, _ => {} } }
// single_arm_match: a single-arm match is not a branch.
pub fn single_arm_match(value: u8) { match value { _ => {} } }
// guarded_match: a two-arm match plus an authored guard is two branches.
pub fn guarded_match(value: Option<u8>) { match value { Some(v) if v > 0 => {}, _ => {} } }
// adjacent_returns: one `if` and two returns separated by their value expressions.
pub fn adjacent_returns(flag: bool) -> u8 { if flag { return 1; } return 2; }
// separated_returns: two `if` branches and two returns in a non-Result body.
pub fn separated_returns(flag: bool, other: bool) -> u8 { if flag { return 1; } if other { return 2; } 3 }
// separated_result_returns: the same shape, but a Result body credits returns in full.
pub fn separated_result_returns(flag: bool, other: bool) -> Result<u8, ()> { if flag { return Ok(1); } if other { return Ok(2); } Ok(3) }
// returns_result: one `if` and one credited return in a Result body.
pub fn returns_result(flag: bool) -> Result<u8, ()> { if flag { return Ok(1); } Ok(2) }
// source_for: `for` desugars to a two-arm match.
pub fn source_for(values: &[u8]) { for value in values { let _ = *value; } }
// source_while: `while` desugars to an `if`.
pub fn source_while(mut count: u8) { while count > 0 { count -= 1; } }
// source_for_if: the desugared `for` match plus an authored `if`.
pub fn source_for_if(values: &[u8]) { for value in values { if *value > 0 {} } }
// try_result: `?` lowers to a two-arm match and one return in a Result body.
pub fn try_result(input: Result<u8, ()>) -> Result<u8, ()> { let value = input?; Ok(value) }
// try_option: `?` on Option halves the credit for its single return.
pub fn try_option(input: Option<u8>) -> Option<u8> { let value = input?; Some(value) }
// closure_body: the closure scores 2 and the enclosing function scores 1.
pub fn closure_body() { let callback = |flag: bool| if flag {}; callback(true); }
// async_if: the coroutine wrapper is not scored; the authored `if` is.
pub async fn async_if(flag: bool) { if flag {} }
// async_await: `await` lowers to a two-arm match that both tools charge.
pub async fn async_await(future: impl core::future::Future<Output = u8>) -> u8 { future.await }
";

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

/// Captured output of one lint invocation.
struct RawRun {
    is_success: bool,
    stdout: Vec<u8>,
    stderr: String,
}

impl RawRun {
    /// Capture one finished command's status and streams.
    fn from_output(output: Output) -> Self {
        Self {
            is_success: output.status.success(),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// Run `args` through Cargo, advising installation when the tool is missing.
///
/// # Errors
///
/// Return an error when the command cannot be spawned or exits unsuccessfully.
fn ensure_tool(description: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("cargo")
        .args(args)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain())
        .output()
        .with_context(|| format!("failed to run `{description}`; is it installed?"))?;
    if !output.status.success() {
        anyhow::bail!(
            "`{description}` failed; install Clippy with `rustup component add clippy` and Dylint \
             with `cargo install cargo-dylint dylint-link`; stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

/// Find the workspace root containing this crate's manifest.
///
/// # Errors
///
/// Return an error when no workspace manifest exists above `CARGO_MANIFEST_DIR`
/// or a candidate directory or manifest cannot be read.
fn workspace_root() -> anyhow::Result<Utf8PathBuf> {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidate = manifest_dir.as_path();
    loop {
        if is_workspace_root(candidate)? {
            return Ok(candidate.to_path_buf());
        }
        candidate = candidate
            .parent()
            .context("workspace root not found above CARGO_MANIFEST_DIR")?;
    }
}

/// Check whether `candidate` contains a workspace manifest.
///
/// # Errors
///
/// Return an error when the directory cannot be opened or its manifest exists
/// but cannot be read; return `false` only when it is absent or not a workspace.
fn is_workspace_root(candidate: &Utf8Path) -> anyhow::Result<bool> {
    let directory = Dir::open_ambient_dir(candidate, ambient_authority())
        .with_context(|| format!("failed to open candidate directory `{candidate}`"))?;
    match directory.read_to_string("Cargo.toml") {
        Ok(manifest) => Ok(manifest.contains("[workspace]")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error)
            .with_context(|| format!("failed to read workspace manifest in `{candidate}`")),
    }
}

/// Create an isolated target directory under the workspace's `target/tests`.
///
/// The system temporary directory is never used for `CARGO_TARGET_DIR`, and the
/// workspace's own `debug` profile directory is never shared with a staged lint
/// library.
///
/// # Errors
///
/// Return an error when the parent cannot be created or the temporary directory
/// cannot be made inside it.
fn private_target_dir(workspace_root: &Utf8Path, label: &str) -> anyhow::Result<TempDir> {
    let workspace = Dir::open_ambient_dir(workspace_root, ambient_authority())
        .context("failed to open the workspace for an isolated target directory")?;
    workspace
        .create_dir_all("target/tests/parity")
        .context("failed to create the isolated target parent directory")?;
    TempDir::new_in(workspace_root.join("target/tests/parity").as_std_path())
        .with_context(|| format!("failed to create the isolated `{label}` target directory"))
}

/// Build the lint library and stage a toolchain-qualified copy for Dylint.
///
/// # Errors
///
/// Return an error when the build fails, the cdylib is absent from the artefact
/// stream, or the staged copy cannot be written.
fn build_and_stage_library(
    workspace_root: &Utf8Path,
    target_dir: &Utf8Path,
) -> anyhow::Result<Utf8PathBuf> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--lib",
            "--quiet",
            "--message-format=json",
            "-p",
            LINT_PACKAGE,
            "--features",
            "dylint-driver",
        ])
        .current_dir(workspace_root.as_std_path())
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .context("failed to execute cargo build for the lint library")?;
    if !output.status.success() {
        anyhow::bail!(
            "lint library build failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let cdylib_path = find_cdylib_in_artefacts(&output.stdout)?;
    let debug_dir = cdylib_path
        .parent()
        .context("staged cdylib has no parent directory")?
        .to_owned();
    stage_toolchain_qualified_library(&cdylib_path, &debug_dir)?;
    Ok(debug_dir)
}

/// Extract the lint package's cdylib path from cargo build JSON output.
///
/// # Errors
///
/// Return an error when the stream cannot be parsed or no cdylib artefact for
/// the lint package is present.
fn find_cdylib_in_artefacts(stdout: &[u8]) -> anyhow::Result<Utf8PathBuf> {
    for parsed in Message::parse_stream(Cursor::new(stdout)) {
        let message = parsed.context("failed to parse cargo build JSON output")?;
        let Message::CompilerArtifact(artefact) = message else {
            continue;
        };
        if artefact.target.name != LINT_PACKAGE || !artefact.target.is_cdylib() {
            continue;
        }
        if let Some(path) = artefact
            .filenames
            .iter()
            .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        {
            return Ok(path.clone());
        }
    }
    anyhow::bail!("cdylib artefact for `{LINT_PACKAGE}` not found in build output")
}

/// Copy the cdylib to a toolchain-qualified filename for Dylint discovery.
///
/// # Errors
///
/// Return an error when the source has no filename or the copy fails.
fn stage_toolchain_qualified_library(
    cdylib_path: &Utf8Path,
    directory: &Utf8Path,
) -> anyhow::Result<()> {
    let toolchain = pinned_toolchain();
    let file_name = cdylib_path
        .file_name()
        .context("staged cdylib has no file name")?;
    let suffix = env::consts::DLL_SUFFIX;
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    );
    let staged_directory = Dir::open_ambient_dir(directory, ambient_authority())
        .context("failed to open the staged library directory")?;
    staged_directory
        .copy(file_name, &staged_directory, &target_name)
        .with_context(|| format!("failed to stage lint library as `{target_name}`"))?;
    Ok(())
}

/// Return the toolchain the lint library is built with.
///
/// `cargo dylint` is told the same toolchain explicitly, so the fixture
/// directory's own toolchain resolution cannot pick a different nightly.
fn pinned_toolchain() -> String {
    env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .unwrap_or_else(|| "unknown-toolchain".to_owned())
}

/// Write the fixture project files beneath `root`.
///
/// # Errors
///
/// Return an error when the root cannot be opened, `src` cannot be created, or
/// any fixture file cannot be written.
fn write_fixture(root: &Utf8Path) -> anyhow::Result<()> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .context("failed to open the fixture root")?;
    directory
        .create_dir_all("src")
        .context("failed to create the fixture `src` directory")?;
    directory.write("Cargo.toml", FIXTURE_MANIFEST)?;
    directory.write("clippy.toml", FIXTURE_CLIPPY_CONFIG)?;
    directory.write("dylint.toml", FIXTURE_DYLINT_CONFIG)?;
    directory.write("src/lib.rs", FIXTURE_SOURCE)?;
    Ok(())
}

/// Run pinned Clippy over the fixture and capture its JSON stream.
///
/// `RUSTFLAGS` is removed so the outer gate's `-D warnings` cannot turn the
/// fixture's measured warnings into a build failure.
///
/// # Errors
///
/// Return an error when the command cannot be spawned.
fn run_clippy(fixture: &Path, target_dir: &Utf8Path) -> anyhow::Result<RawRun> {
    let output = Command::new("cargo")
        .args([
            "clippy",
            "--lib",
            "--message-format=json",
            "--",
            "-W",
            CLIPPY_CODE,
        ])
        .current_dir(fixture)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain())
        .env_remove("RUSTFLAGS")
        .env_remove("DYLINT_LIBRARY_PATH")
        .env_remove("DYLINT_RUSTFLAGS")
        .output()
        .context("failed to execute cargo clippy")?;
    Ok(RawRun::from_output(output))
}

/// Run this lint through `cargo dylint` and capture its JSON stream.
///
/// `DYLINT_RUSTFLAGS=-D warnings` denies each finding, so a diagnostic makes
/// the command fail while the JSON stream stays complete.
///
/// # Errors
///
/// Return an error when the command cannot be spawned.
fn run_whitaker(
    fixture: &Path,
    target_dir: &Utf8Path,
    library_dir: &Utf8Path,
) -> anyhow::Result<RawRun> {
    let output = Command::new("cargo")
        .args(["dylint", "--all", "--", "--message-format", "json"])
        .current_dir(fixture)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("DYLINT_LIBRARY_PATH", library_dir)
        .env("DYLINT_RUSTFLAGS", "-D warnings")
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain())
        .env_remove("RUSTFLAGS")
        .output()
        .context("failed to execute cargo dylint")?;
    Ok(RawRun::from_output(output))
}

/// Return the `compiler-message` values from a captured cargo JSON stream.
///
/// Non-JSON lines are skipped because Cargo may interleave plain text. A
/// wholesale message-format change still fails the minimum-count assertion.
fn compiler_messages(raw: &RawRun) -> Vec<Value> {
    raw.stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
        .filter(|message| message.get("reason").and_then(Value::as_str) == Some("compiler-message"))
        .collect()
}

/// Collect `(anchor line, score)` pairs for diagnostics carrying `code`.
///
/// # Errors
///
/// Return an error when a selected diagnostic has no primary span line or its
/// rendered message does not carry a parseable score.
fn lint_scores(raw: &RawRun, code: &str) -> anyhow::Result<Vec<(u32, u64)>> {
    let mut scores = Vec::new();
    for message in compiler_messages(raw) {
        let diagnostic = message
            .get("message")
            .context("compiler message has no `message` field")?;
        let is_under_test = diagnostic
            .get("code")
            .and_then(|entry| entry.get("code"))
            .and_then(Value::as_str)
            == Some(code);
        if !is_under_test {
            continue;
        }
        let Some(span) = diagnostic
            .get("spans")
            .and_then(Value::as_array)
            .and_then(|spans| {
                spans
                    .iter()
                    .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
            })
        else {
            continue;
        };
        let line_start = span
            .get("line_start")
            .and_then(Value::as_u64)
            .context("primary span has no `line_start`")?;
        let line = u32::try_from(line_start).context("anchor line does not fit in `u32`")?;
        let rendered = diagnostic
            .get("message")
            .and_then(Value::as_str)
            .context("diagnostic has no rendered message")?;
        let score = parse_score(rendered).with_context(|| {
            format!("could not parse a cognitive complexity score from `{rendered}`")
        })?;
        scores.push((line, score));
    }
    scores.sort_unstable();
    Ok(scores)
}

/// Read the leading digits after `"cognitive complexity of "`.
///
/// Clippy wraps the score in parentheses (`(N/M)`); Whitaker does not. An
/// optional opening parenthesis is skipped, and absent digits yield `None` so a
/// message-format change fails loudly rather than silently scoring zero.
fn parse_score(message_text: &str) -> Option<u64> {
    let (_, rest) = message_text.split_once(SCORE_MARKER)?;
    let digits_start = rest.strip_prefix('(').unwrap_or(rest);
    let digit_count = digits_start.bytes().take_while(u8::is_ascii_digit).count();
    digits_start.get(..digit_count)?.parse().ok()
}

/// Find the 1-based line of the first fixture line containing `needle`.
///
/// # Errors
///
/// Return an error when no line matches or the line number does not fit `u32`.
fn fixture_line(needle: &str) -> anyhow::Result<u32> {
    let index = FIXTURE_SOURCE
        .lines()
        .position(|line| line.contains(needle))
        .with_context(|| format!("fixture source has no line containing `{needle}`"))?;
    u32::try_from(index + 1).context("fixture line number does not fit in `u32`")
}

/// Return a UTF-8 view of a temporary directory's path.
///
/// # Errors
///
/// Return an error when the path cannot be represented as UTF-8.
fn utf8_temp_path(directory: &TempDir) -> anyhow::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("temporary path is not UTF-8: {}", path.display()))
}

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
