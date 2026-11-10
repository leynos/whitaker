//! Feature-graph matrix plumbing for the logging-bridge parity test.
//!
//! `ExecPlan` V5 discharges R3/R4 outside the lint: the same four `tracing`
//! calls must score one whether or not the `log` feature pulls the `log` crate
//! into the build. Pinned Clippy measures roughly 29 in the `log`-enabled graph
//! because `tracing`'s compatibility macros expand differently; this lint must
//! be insensitive to that expansion.
//!
//! The module is split across `projects`, `runs`, and `diagnostics` so each
//! file stays inside the repository's line budget. It owns dependency pinning
//! from the workspace lockfile, generation of the two standalone projects,
//! their nested `cargo tree` and `cargo dylint` invocations, diagnostic
//! normalization, and the retained run receipts. The test target itself stays
//! a readable assertion entrypoint.

mod diagnostics;
mod projects;
mod runs;

pub(super) use diagnostics::{
    assert_baseline_score,
    assert_four_logging_calls,
    assert_log_feature_graph,
    assert_single_diagnostic,
    assert_vectors_match,
    single_score,
};
pub(super) use projects::{
    assert_versions_agree,
    generate_lockfile,
    workspace_versions,
    write_project,
};
pub(super) use runs::{MatrixContext, private_target_dir_in, run_project};

/// The identical source both generated projects compile.
///
/// Four `tracing` calls with no authored branching must score the authoring
/// baseline. Keeping the source identical means the normalized diagnostics may
/// differ only by the generated project root.
pub(super) const PROJECT_SOURCE: &str = concat!(
    "pub fn four_logging_calls(value: u8) -> u8 {\n",
    "    tracing::info!(value, \"first\");\n",
    "    tracing::info!(value, \"second\");\n",
    "    tracing::info!(value, \"third\");\n",
    "    tracing::info!(value, \"fourth\");\n",
    "    value\n",
    "}\n",
);

/// Which dependency graph a generated matrix project selects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectMode {
    /// `tracing` with its default features disabled and no `log` bridge.
    WithoutLog,
    /// `tracing` with the `log` feature, which adds the `log` crate.
    WithLog,
}

impl ProjectMode {
    /// Return the Cargo package name of this mode's generated project.
    pub(super) const fn package_name(self) -> &'static str {
        match self {
            Self::WithoutLog => "logging_without_log",
            Self::WithLog => "logging_with_log",
        }
    }

    /// Return the path- and receipt-safe label for this mode.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::WithoutLog => "without-log",
            Self::WithLog => "with-log",
        }
    }

    /// Render this mode's `tracing` feature list as manifest array contents.
    const fn tracing_features(self) -> &'static str {
        match self {
            Self::WithoutLog => "\"attributes\"",
            Self::WithLog => "\"attributes\", \"log\"",
        }
    }
}
