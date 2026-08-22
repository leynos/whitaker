//! Operator-facing progress messages for managed workspace operations.
//!
//! This module keeps CLI reporting separate from checkout mutation. It reports
//! the action recorded by workspace preparation and the resolved pin.

use std::io::Write;
use whitaker_installer::cli::InstallArgs;
use whitaker_installer::git::CommitSha;
use whitaker_installer::output::write_stderr_line;
use whitaker_installer::workspace::{WorkspaceAction, WorkspaceCheckout};

/// Reports the workspace action and requested pin selected during preparation.
pub(super) fn report_workspace_progress(
    args: &InstallArgs,
    checkout: &WorkspaceCheckout,
    stderr: &mut dyn Write,
) {
    if args.quiet {
        return;
    }

    match &checkout.action {
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
fn short_commit(commit: &CommitSha) -> &str {
    let commit = commit.as_str();
    let end = commit.len().min(12);
    &commit[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use rstest::rstest;

    const COMMIT: &str = "abc1234567890000000000000000000000000000";

    fn pinned_checkout() -> WorkspaceCheckout {
        let root = Utf8PathBuf::from("/managed/whitaker");
        WorkspaceCheckout {
            root: root.clone(),
            pinned_commit: Some(CommitSha::try_from(COMMIT).expect("full test commit SHA")),
            detached_commit: None,
            action: WorkspaceAction::UseExisting(root),
        }
    }

    fn checkout(action: WorkspaceAction) -> WorkspaceCheckout {
        WorkspaceCheckout {
            root: Utf8PathBuf::from("/managed/whitaker"),
            pinned_commit: None,
            detached_commit: None,
            action,
        }
    }

    #[rstest]
    #[case::clone(
        WorkspaceAction::CloneTo(Utf8PathBuf::from("/managed/whitaker")),
        "workspace_progress_reports_clone_message"
    )]
    #[case::update(
        WorkspaceAction::UpdateAt(Utf8PathBuf::from("/managed/whitaker")),
        "workspace_progress_reports_update_message"
    )]
    fn workspace_progress_reports_exact_action_message(
        #[case] action: WorkspaceAction,
        #[case] snapshot_name: &str,
    ) {
        let mut output = Vec::new();

        report_workspace_progress(&InstallArgs::default(), &checkout(action), &mut output);

        insta::assert_snapshot!(
            snapshot_name,
            String::from_utf8(output).expect("UTF-8 output")
        );
    }

    #[test]
    fn workspace_progress_reports_requested_pin_message() {
        let args = InstallArgs {
            git_ref: Some("v0.2.5".to_owned()),
            ..InstallArgs::default()
        };
        let mut output = Vec::new();

        report_workspace_progress(
            &args,
            &checkout(WorkspaceAction::UseExisting(Utf8PathBuf::from(
                "/managed/whitaker",
            ))),
            &mut output,
        );

        insta::assert_snapshot!(
            "workspace_progress_reports_requested_pin_message",
            String::from_utf8(output).expect("UTF-8 output")
        );
    }

    #[test]
    fn workspace_progress_is_silent_in_quiet_mode() {
        let args = InstallArgs {
            quiet: true,
            ..InstallArgs::default()
        };
        let mut output = Vec::new();

        report_workspace_progress(
            &args,
            &checkout(WorkspaceAction::CloneTo(Utf8PathBuf::from(
                "/managed/whitaker",
            ))),
            &mut output,
        );

        let output = String::from_utf8(output).expect("UTF-8 output");

        insta::assert_snapshot!(
            "workspace_progress_is_silent_in_quiet_mode",
            format!("{output:?}")
        );
    }

    #[rstest]
    #[case::requested_ref(Some("v0.2.5"), "pinned_checkout_reports_requested_ref")]
    #[case::commit_fallback(None, "pinned_checkout_reports_commit_fallback")]
    fn pinned_checkout_reports_exact_message(
        #[case] git_ref: Option<&str>,
        #[case] snapshot_name: &str,
    ) {
        let mut output = Vec::new();

        report_pinned_checkout(false, git_ref, &pinned_checkout(), &mut output);

        insta::assert_snapshot!(
            snapshot_name,
            String::from_utf8(output).expect("UTF-8 output")
        );
    }

    #[test]
    fn pinned_checkout_is_silent_in_quiet_mode() {
        let mut output = Vec::new();

        report_pinned_checkout(true, Some("v0.2.5"), &pinned_checkout(), &mut output);

        let output = String::from_utf8(output).expect("UTF-8 output");

        insta::assert_snapshot!(
            "pinned_checkout_is_silent_in_quiet_mode",
            format!("{output:?}")
        );
    }
}
