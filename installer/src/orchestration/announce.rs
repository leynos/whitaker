//! User-facing notices the orchestration emits before doing the work.
//!
//! These belong outside the flow module because they are pure formatting: each
//! takes an already-decided plan and writes one line, which keeps the caller
//! readable and lets the message shape be changed without touching the steps.

use std::io::Write;

use camino::Utf8PathBuf;

use crate::{
    dirs::BaseDirs,
    error::{InstallerError, Result},
    install_metrics::InstallMode,
    output::write_stderr_line,
    workspace::WorkspacePlan,
};

/// Announce, on stdout, whether the suite came from a published artefact.
///
/// stdout rather than stderr, and a fixed shape rather than prose, because a
/// caller reads it. `install-whitaker` previously had to infer the path from
/// the wording of a fallback notice, which meant a rephrasing could silently
/// turn a source build into a reported success.
///
/// # Errors
///
/// Returns the write failure when the notice cannot reach stdout.
pub(super) fn report_suite_source(install_mode: InstallMode, stdout: &mut dyn Write) -> Result<()> {
    let source = match install_mode {
        InstallMode::Download => "prebuilt",
        InstallMode::Build => "source",
    };
    writeln!(stdout, "whitaker-installer: suite-source={source}")
        .map_err(|error| InstallerError::WriteFailed { source: error })
}

/// Announce, on stderr, the workspace action `ensure_workspace` will take.
///
/// The notice is advisory, so an unavailable clone directory or working
/// directory suppresses it rather than failing the run.
pub(super) fn report_workspace_action(
    dirs: &dyn BaseDirs,
    plan: &WorkspacePlan,
    stderr: &mut dyn Write,
) {
    use crate::workspace::{WorkspaceAction, clone_directory, decide_workspace_action};

    let Some(clone_dir) = clone_directory(dirs) else {
        return;
    };
    let Some(cwd) = std::env::current_dir()
        .ok()
        .and_then(|p| Utf8PathBuf::try_from(p).ok())
    else {
        return;
    };

    match decide_workspace_action(&cwd, &clone_dir, plan.should_update) {
        WorkspaceAction::CloneTo(dir) => {
            write_stderr_line(stderr, format!("Cloning Whitaker repository to {dir}..."));
        }
        WorkspaceAction::UpdateAt(dir) => {
            write_stderr_line(stderr, format!("Updating Whitaker repository at {dir}..."));
        }
        WorkspaceAction::UseCurrentDir(_) | WorkspaceAction::UseExisting(_) => {}
    }
    if let Some(reference) = plan.suite_ref.as_ref() {
        write_stderr_line(stderr, format!("Pinning lint suite to {reference}..."));
    }
}
