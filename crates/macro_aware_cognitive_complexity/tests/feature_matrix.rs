//! Logging feature-graph matrix for the macro-aware cognitive-complexity lint.
//!
//! `ExecPlan` V5 discharges R3/R4 outside the lint: the same four `tracing`
//! calls must score one whether or not `tracing`'s `log` feature pulls the
//! `log` crate into the build. Pinned Clippy measures about 29 in the
//! `log`-enabled graph because the compatibility macros expand differently;
//! Whitaker must classify that expansion as generated and stay at the
//! authoring baseline.
//!
//! Two standalone projects are generated with identical source and an
//! identical pinned `tracing` version. Only the `log` feature differs. Each
//! project's `cargo tree -e features` proves the graph difference, and each
//! lint run's normalized diagnostics must then be byte-identical with a score
//! of one.
//!
//! The test is ignored because it resolves registry dependencies and builds
//! two cold dependency graphs; it is run explicitly for milestone acceptance.
//! The Nextest `serial-feature-matrix` group keeps it off the nested-build
//! lanes, and `serial_test::serial` stops a stray direct `cargo test` from
//! racing the UI harnesses.

#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]
#![cfg(feature = "dylint-driver")]

#[path = "support/feature_matrix/mod.rs"]
mod feature_matrix_support;
// The shared module also serves the parity target, so every helper it owns is
// live there even when this target uses only the nested-build subset.
#[expect(
    dead_code,
    reason = "the shared support module is also compiled by the parity target"
)]
mod support;

use serial_test::serial;

use self::{
    feature_matrix_support::{
        MatrixContext,
        ProjectMode,
        assert_baseline_scores,
        assert_four_logging_calls,
        assert_instrumented_async_fn,
        assert_log_feature_graph,
        assert_probe_diagnostics,
        assert_vectors_match,
        assert_versions_agree,
        generate_lockfile,
        private_target_dir_in,
        run_project,
        workspace_versions,
        write_project,
    },
    support::{build_and_stage_library, ensure_tool, utf8_temp_path, workspace_root},
};

/// Assert the same diagnostics and score with and without the `log` feature.
///
/// # Errors
///
/// Return an error when the tools are missing, the workspace or generated
/// projects cannot be prepared, or a nested run cannot be captured.
#[test]
#[ignore = "resolves registry dependencies; run explicitly for milestone acceptance"]
#[serial]
fn feature_matrix_log_feature_does_not_change_score() -> anyhow::Result<()> {
    ensure_tool("cargo dylint --version", &["dylint", "--version"])?;
    let workspace = workspace_root()?;
    let versions = workspace_versions(&workspace)?;

    let library_target = private_target_dir_in(&workspace, "feature_matrix/library", "library-")?;
    let library_dir = build_and_stage_library(&workspace, &utf8_temp_path(&library_target)?)?;

    let fixture = private_target_dir_in(&workspace, "feature_matrix/projects", "projects-")?;
    let fixture_root = utf8_temp_path(&fixture)?;
    let plain_root = write_project(&fixture_root, ProjectMode::WithoutLog, &versions)?;
    let with_log_root = write_project(&fixture_root, ProjectMode::WithLog, &versions)?;
    for project_root in [&plain_root, &with_log_root] {
        generate_lockfile(project_root)?;
    }
    assert_versions_agree(&plain_root, &with_log_root, &versions)?;

    let context = MatrixContext {
        workspace: &workspace,
        library_dir: &library_dir,
        versions: &versions,
    };
    let plain = run_project(&context, &plain_root, ProjectMode::WithoutLog)?;
    let with_log = run_project(&context, &with_log_root, ProjectMode::WithLog)?;

    assert_log_feature_graph(&plain, &with_log);
    assert_probe_diagnostics(&plain.diagnostics, ProjectMode::WithoutLog);
    assert_probe_diagnostics(&with_log.diagnostics, ProjectMode::WithLog);
    assert_vectors_match(&plain.diagnostics, &with_log.diagnostics);
    assert_baseline_scores(&plain.diagnostics)?;
    assert_four_logging_calls();
    assert_instrumented_async_fn();
    Ok(())
}
