//! Verify failed dependency queries stop before the installation decision.

use super::dependency_check::ensure_dylint_tools_with_query;
use crate::error::InstallerError;
use rstest::rstest;
use std::cell::Cell;

/// Verify failed availability queries stop before tool installation begins.
#[rstest]
#[case::path_lookup(InstallerError::DependencyCheck {
    tool: "dylint-link",
    reason: "PATH directory could not be opened".to_owned(),
})]
#[case::metadata_lookup(InstallerError::DependencyCheck {
    tool: "dylint-link",
    reason: "PATH candidate metadata could not be read".to_owned(),
})]
#[case::command_query(InstallerError::Io(std::io::Error::other(
    "dependency command could not be started",
)))]
fn query_failures_do_not_start_tool_installation(#[case] query_error: InstallerError) {
    let install_started = Cell::new(false);
    let mut stderr = Vec::new();
    let expected_message = query_error.to_string();

    let result = ensure_dylint_tools_with_query(
        false,
        &mut stderr,
        || Err(query_error),
        |_, _| {
            install_started.set(true);
            Ok(())
        },
    );

    assert_eq!(
        result
            .expect_err("a failed availability query must stop the flow")
            .to_string(),
        expected_message,
    );
    assert!(!install_started.get());
    assert!(stderr.is_empty());
}
