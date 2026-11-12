//! Nested Cargo runs for the feature-graph matrix.
//!
//! Every run keeps the shared default Cargo cache and the compiler wrappers so
//! admission still applies; only the outer gate's `RUSTFLAGS` and the target
//! directory are scoped. Captured streams are preserved both beside the
//! generated project and under `target/tests/feature_matrix/receipts`.

use std::process::Command;

use anyhow::Context as _;
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use serde_json::Value;
use tempfile::TempDir;

use super::{ProjectMode, projects::LockedVersions};
use crate::support::{
    LINT_CODE,
    RawRun,
    compiler_messages,
    pinned_toolchain,
    run_whitaker,
    utf8_temp_path,
};

/// The captured result of running one generated project through the matrix.
pub(crate) struct ProjectRun {
    /// The diagnostics normalized against the generated project root.
    pub(crate) diagnostics: Vec<super::diagnostics::NormalizedDiagnostic>,
    /// Whether `cargo tree -e features` lists the `log` crate.
    pub(crate) tree_lists_log: bool,
    /// Whether `cargo tree -e features` enables `tracing`'s `log` feature.
    pub(crate) tree_lists_tracing_log_feature: bool,
}

/// One project run's captured streams, grouped for preservation on disk.
pub(crate) struct RunStreams<'a> {
    /// The `cargo dylint` capture.
    pub(crate) dylint: &'a RawRun,
    /// The `cargo tree -e features` capture.
    pub(crate) tree: &'a RawRun,
}

/// Shared inputs for every generated project's nested runs.
pub(crate) struct MatrixContext<'a> {
    /// The workspace root, used to place private temporary directories.
    pub(crate) workspace: &'a Utf8Path,
    /// The private staged lint library Dylint must load.
    pub(crate) library_dir: &'a Utf8Path,
    /// The pinned dependency versions.
    pub(crate) versions: &'a LockedVersions,
}

/// Create an isolated directory under `target/tests/<category>`.
///
/// This lives beside the matrix rather than in `support` because the shared
/// module is already near its line budget; the logic deliberately mirrors
/// `support::private_target_dir`, which stays parity-specific.
///
/// # Errors
///
/// Return an error when the parent directory cannot be created or the
/// temporary directory cannot be allocated.
pub(crate) fn private_target_dir_in(
    workspace_root: &Utf8Path,
    category: &str,
    label: &str,
) -> anyhow::Result<TempDir> {
    let workspace = Dir::open_ambient_dir(workspace_root, ambient_authority())
        .context("failed to open the workspace for an isolated target directory")?;
    let relative = format!("target/tests/{category}");
    workspace
        .create_dir_all(&relative)
        .context("failed to create the isolated target parent directory")?;
    TempDir::new_in(workspace_root.join(&relative).as_std_path())
        .with_context(|| format!("failed to create the isolated `{label}` target directory"))
}

/// Run `cargo tree -e features` in one generated project.
///
/// The pinned toolchain and `RUSTFLAGS` removal match the dylint runs; the
/// compiler wrapper and shared Cargo cache are inherited so admission applies.
///
/// # Errors
///
/// Return an error only when the command cannot be spawned.
pub(crate) fn run_cargo_tree(
    project_root: &Utf8Path,
    target_dir: &Utf8Path,
) -> anyhow::Result<RawRun> {
    let output = Command::new("cargo")
        .args(["tree", "-e", "features"])
        .current_dir(project_root.as_std_path())
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain()?)
        .env_remove("RUSTFLAGS")
        .output()
        .context("failed to execute `cargo tree -e features`")?;
    Ok(RawRun::from_output(output))
}

/// Return whether a `cargo tree` run lists `name v<version>` as a node.
///
/// Tree drawing prefixes every node with box-drawing characters and
/// indentation, so the prefix is stripped before the node text is compared. A
/// repeated node carries a trailing ` (*)`, which the comparison tolerates.
pub(crate) fn tree_lists_package(raw: &RawRun, name: &str, version: &str) -> bool {
    let expected = format!("{name} v{version}");
    tree_nodes(raw).any(|node| {
        node == expected
            || node
                .strip_prefix(expected.as_str())
                .is_some_and(|remainder| remainder == " (*)")
    })
}

/// Return whether a `cargo tree` run enables `package`'s `feature` edge.
///
/// A direct dependency on the same crate is not the same evidence: the matrix
/// adds `log` to the manifest as well as enabling `tracing`'s `log` feature, so
/// package presence alone would still hold if the feature were dropped and the
/// two projects compiled the same expansion.
pub(crate) fn tree_lists_feature_edge(raw: &RawRun, package: &str, feature: &str) -> bool {
    let expected = format!("{package} feature \"{feature}\"");
    tree_nodes(raw).any(|node| node == expected)
}

/// Yield each tree node with its drawing prefix and indentation removed.
fn tree_nodes(raw: &RawRun) -> impl Iterator<Item = &str> {
    raw.stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| std::str::from_utf8(line).ok())
        .map(|line| {
            line.trim_start_matches(|character: char| {
                character.is_whitespace() || matches!(character, '│' | '├' | '└' | '─')
            })
        })
}

/// Run one generated project through `cargo tree` and `cargo dylint`.
///
/// The project's target directory and preserved logs are private to this run.
///
/// # Errors
///
/// Return an error when the runs cannot be spawned, their logs cannot be
/// written, or the diagnostics cannot be normalized.
pub(crate) fn run_project(
    context: &MatrixContext<'_>,
    project_root: &Utf8Path,
    mode: ProjectMode,
) -> anyhow::Result<ProjectRun> {
    let category = format!("feature_matrix/{}", mode.package_name());
    let target = private_target_dir_in(context.workspace, &category, "target-")?;
    let target_dir = utf8_temp_path(&target)?;
    let tree = run_cargo_tree(project_root, &target_dir)?;
    let dylint = run_whitaker(project_root.as_std_path(), &target_dir, context.library_dir)?;
    let streams = RunStreams {
        dylint: &dylint,
        tree: &tree,
    };
    persist_logs(context, project_root, mode, &streams)?;
    ensure_tree_succeeded(&tree, mode)?;
    assert_denied(&dylint, mode);
    assert_only_lint_errors(&dylint);
    let diagnostics = super::diagnostics::collect_normalized(&dylint, project_root)?;
    Ok(ProjectRun {
        diagnostics,
        tree_lists_log: tree_lists_package(&tree, "log", &context.versions.log),
        tree_lists_tracing_log_feature: tree_lists_feature_edge(&tree, "tracing", "log"),
    })
}

/// Reject a failed `cargo tree` run before its output is read as evidence.
///
/// An unsuccessful run usually has empty stdout, which would otherwise read as
/// "the `log` crate is absent" and let the plain project's assertion pass
/// without proving anything. The streams are already preserved by the caller,
/// so the error can quote the captured stderr.
///
/// # Errors
///
/// Return an error when the run did not exit successfully.
fn ensure_tree_succeeded(tree: &RawRun, mode: ProjectMode) -> anyhow::Result<()> {
    anyhow::ensure!(
        tree.is_success,
        "`cargo tree -e features` failed for {}:\n{}",
        mode.label(),
        tree.stderr
    );
    Ok(())
}

/// Assert `cargo dylint` exited unsuccessfully because the finding is denied.
pub(crate) fn assert_denied(dylint: &RawRun, mode: ProjectMode) {
    assert!(
        !dylint.is_success,
        "{} must fail because `-D warnings` denies the lint finding\nstderr:\n{}",
        mode.label(),
        dylint.stderr
    );
}

/// Assert every coded error-level message names this lint.
///
/// Rustc also emits an uncoded `aborting due to N previous errors` summary at
/// error level, so only a foreign code is evidence of an unrelated failure.
fn assert_only_lint_errors(dylint: &RawRun) {
    for message in compiler_messages(dylint) {
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
            dylint.stderr
        );
    }
}

/// Preserve one run's raw streams beside the project and under `target/tests`.
///
/// The project copy survives until the caller drops its `TempDir`. The receipt
/// copy outlives the test, so a failed acceptance run can be inspected
/// afterwards. Receipts are workspace-relative rather than shared under `/tmp`:
/// two worktrees may run this test at the same time, and neither may overwrite
/// the other's evidence.
///
/// # Errors
///
/// Return an error when either location cannot be opened or written.
fn persist_logs(
    context: &MatrixContext<'_>,
    project_root: &Utf8Path,
    mode: ProjectMode,
    streams: &RunStreams<'_>,
) -> anyhow::Result<()> {
    let fixture = Dir::open_ambient_dir(project_root, ambient_authority())
        .with_context(|| format!("failed to open `{project_root}` to preserve run logs"))?;
    write_streams(&fixture, streams)?;

    let receipts_path = format!("target/tests/feature_matrix/receipts/{}", mode.label());
    let workspace =
        Dir::open_ambient_dir(context.workspace, ambient_authority()).with_context(|| {
            format!(
                "failed to open `{}` for the run receipts",
                context.workspace
            )
        })?;
    workspace
        .create_dir_all(&receipts_path)
        .with_context(|| format!("failed to create the receipt directory `{receipts_path}`"))?;
    let receipts = workspace
        .open_dir(&receipts_path)
        .with_context(|| format!("failed to open the receipt directory `{receipts_path}`"))?;
    write_streams(&receipts, streams)
}

/// Write one run's four captured streams into `directory`.
///
/// # Errors
///
/// Return an error when a stream cannot be written.
fn write_streams(directory: &Dir, streams: &RunStreams<'_>) -> anyhow::Result<()> {
    directory
        .write("dylint.stdout.json", &streams.dylint.stdout)
        .context("failed to preserve dylint stdout")?;
    directory
        .write("dylint.stderr.txt", streams.dylint.stderr.as_bytes())
        .context("failed to preserve dylint stderr")?;
    directory
        .write("tree.stdout.txt", &streams.tree.stdout)
        .context("failed to preserve cargo tree stdout")?;
    directory
        .write("tree.stderr.txt", streams.tree.stderr.as_bytes())
        .context("failed to preserve cargo tree stderr")
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectMode,
        RawRun,
        ensure_tree_succeeded,
        tree_lists_feature_edge,
        tree_lists_package,
    };

    /// Build a synthetic `cargo tree` capture from its rendered stdout.
    fn tree_run(stdout: &str) -> RawRun {
        RawRun {
            is_success: true,
            stdout: stdout.as_bytes().to_vec(),
            stderr: String::new(),
        }
    }

    /// A direct `log` dependency must not be mistaken for the feature edge.
    ///
    /// This is the case a package-presence check cannot distinguish: `log` is
    /// in the graph, but `tracing`'s `log` feature is not, so the projects'
    /// expansions would not differ for the reason the matrix claims.
    #[test]
    fn direct_log_dependency_is_not_a_tracing_feature_edge() {
        let run = tree_run(concat!(
            "logging_with_log v0.1.0 (/tmp/logging_with_log)\n",
            "├── log v0.4.35\n",
            "└── tracing v0.1.44\n",
            "    └── tracing-core v0.1.36\n",
        ));
        assert!(tree_lists_package(&run, "log", "0.4.35"));
        assert!(
            !tree_lists_feature_edge(&run, "tracing", "log"),
            "a direct `log` dependency is not the `tracing` `log` feature edge"
        );
    }

    /// A feature node nested under a drawn prefix must still be matched.
    #[test]
    fn nested_tracing_log_feature_edge_is_detected() {
        let run = tree_run(concat!(
            "logging_with_log v0.1.0 (/tmp/logging_with_log)\n",
            "└── tracing feature \"attributes\"\n",
            "    └── tracing v0.1.44\n",
            "        ├── tracing feature \"log\"\n",
            "        │   └── log v0.4.35\n",
            "        └── tracing-core v0.1.36\n",
        ));
        assert!(tree_lists_feature_edge(&run, "tracing", "log"));
    }

    /// A failed `cargo tree` run must not be read as an empty graph.
    #[test]
    fn failed_tree_run_is_rejected() {
        let failed = RawRun {
            is_success: false,
            stdout: Vec::new(),
            stderr: "error: failed to load manifest".to_owned(),
        };
        let error = ensure_tree_succeeded(&failed, ProjectMode::WithLog)
            .expect_err("a failed `cargo tree` run must be rejected");
        assert!(
            error.to_string().contains("failed to load manifest"),
            "the error must quote the captured stderr, got: {error}"
        );
        assert!(ensure_tree_succeeded(&tree_run(""), ProjectMode::WithLog).is_ok());
    }
}
