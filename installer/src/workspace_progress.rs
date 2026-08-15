//! Operator-facing progress messages for managed workspace operations.
//!
//! This module keeps CLI reporting separate from checkout mutation. It reports
//! the action recorded by workspace preparation and the resolved pin.

use std::io::Write;
use whitaker_installer::cli::InstallArgs;
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
fn short_commit(commit: &str) -> &str {
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
            pinned_commit: Some(COMMIT.to_owned()),
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
        "Cloning Whitaker repository to /managed/whitaker...\n"
    )]
    #[case::update(
        WorkspaceAction::UpdateAt(Utf8PathBuf::from("/managed/whitaker")),
        "Updating Whitaker repository at /managed/whitaker...\n"
    )]
    fn workspace_progress_reports_exact_action_message(
        #[case] action: WorkspaceAction,
        #[case] expected: &str,
    ) {
        let mut output = Vec::new();

        report_workspace_progress(&InstallArgs::default(), &checkout(action), &mut output);

        assert_eq!(String::from_utf8(output).expect("UTF-8 output"), expected);
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

        assert_eq!(
            String::from_utf8(output).expect("UTF-8 output"),
            "Pinning Whitaker suite to v0.2.5...\n"
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

        assert!(output.is_empty());
    }

    #[rstest]
    #[case::requested_ref(Some("v0.2.5"), "Pinned Whitaker suite to v0.2.5 (abc123456789).\n")]
    #[case::commit_fallback(
        None,
        "Pinned Whitaker suite to abc1234567890000000000000000000000000000 (abc123456789).\n"
    )]
    fn pinned_checkout_reports_exact_message(
        #[case] git_ref: Option<&str>,
        #[case] expected: &str,
    ) {
        let mut output = Vec::new();

        report_pinned_checkout(false, git_ref, &pinned_checkout(), &mut output);

        assert_eq!(String::from_utf8(output).expect("UTF-8 output"), expected);
    }

    #[test]
    fn pinned_checkout_is_silent_in_quiet_mode() {
        let mut output = Vec::new();

        report_pinned_checkout(true, Some("v0.2.5"), &pinned_checkout(), &mut output);

        assert!(output.is_empty());
    }
}
