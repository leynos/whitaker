//! Operator-facing progress messages for managed workspace operations.
//!
//! This module keeps CLI reporting separate from checkout mutation. It predicts
//! the action before installation and reports the resolved pin afterwards.

use camino::Utf8PathBuf;
use std::io::Write;
use whitaker_installer::cli::InstallArgs;
use whitaker_installer::dirs::BaseDirs;
use whitaker_installer::output::write_stderr_line;
use whitaker_installer::workspace::{
    WorkspaceAction, WorkspaceCheckout, clone_directory, decide_workspace_action,
};

/// Reports the workspace action and requested pin before the operation starts.
pub(super) fn report_workspace_progress(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    stderr: &mut dyn Write,
) {
    if args.quiet {
        return;
    }
    let Some(clone_dir) = clone_directory(dirs) else {
        return;
    };
    let Some(cwd) = std::env::current_dir()
        .ok()
        .and_then(|path| Utf8PathBuf::try_from(path).ok())
    else {
        return;
    };

    match decide_workspace_action(&cwd, &clone_dir, !args.no_update) {
        WorkspaceAction::CloneTo(dir) => {
            write_stderr_line(stderr, format!("Cloning Whitaker repository to {dir}..."));
        }
        WorkspaceAction::UpdateAt(dir) => {
            write_stderr_line(stderr, format!("Updating Whitaker repository at {dir}..."));
        }
        WorkspaceAction::UseCurrentDir(_) | WorkspaceAction::UseExisting(_) => {}
    }

    if let Some(git_ref) = &args.git_ref {
        write_stderr_line(stderr, format!("Pinning Whitaker suite to {git_ref}..."));
    }
}

/// Reports the resolved commit after a pinned checkout succeeds.
pub(super) fn report_pinned_checkout(
    quiet: bool,
    git_ref: Option<&str>,
    checkout: &WorkspaceCheckout,
    stderr: &mut dyn Write,
) {
    if quiet {
        return;
    }
    let Some(commit) = &checkout.pinned_commit else {
        return;
    };

    write_stderr_line(
        stderr,
        format!(
            "Pinned Whitaker suite to {} ({}).",
            git_ref.unwrap_or(commit.as_str()),
            short_commit(commit)
        ),
    );
}

/// Abbreviates a commit SHA to its leading 12 characters for display.
fn short_commit(commit: &str) -> &str {
    let end = commit.len().min(12);
    &commit[..end]
}
