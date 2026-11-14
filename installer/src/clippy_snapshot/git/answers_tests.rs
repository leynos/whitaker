//! Integrity diagnostics name the failed check even without Git's text.
//!
//! Policy accepts or rejects the finished diagnostic, so the adapter must
//! supply one for every `fsck` outcome rather than an empty string.

use super::integrity_diagnostic;

#[test]
fn a_successful_check_needs_no_diagnostic() {
    assert_eq!(integrity_diagnostic(true, b"error: ignored\n"), "");
}

#[test]
fn a_failed_check_reports_what_git_said() {
    assert_eq!(
        integrity_diagnostic(false, b"error: object missing\n"),
        "git fsck --full failed: error: object missing"
    );
}

#[test]
fn a_failed_check_without_a_diagnostic_still_names_the_command() {
    assert_eq!(
        integrity_diagnostic(false, b"\n"),
        "git fsck --full failed without a diagnostic"
    );
}
