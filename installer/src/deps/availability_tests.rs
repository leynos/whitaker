//! Verify dependency queries preserve failure state without emitting events.

use super::{
    BinaryLocator, PathScanFailure, PathScanFailureCategory, PathScanResult,
    check_dylint_tools_with_locator,
};
use crate::test_utils::{
    ExpectedCall, StubExecutor,
    dependency_binary_helpers::{cargo_dylint_check, dylint_link_install_list_check},
    failure_output,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::Subscriber;
use tracing_subscriber::{Layer, layer::Context, prelude::*};

struct FixedBinaryLocator {
    binary: Option<PathBuf>,
    failure: Option<PathScanFailureCategory>,
}

impl BinaryLocator for FixedBinaryLocator {
    /// Return the configured fixture result for the requested executable.
    fn find_binary(&self, binary_name: &str) -> PathScanResult {
        let failures = self
            .failure
            .map(|category| {
                vec![PathScanFailure {
                    binary_name: binary_name.to_owned(),
                    category,
                }]
            })
            .unwrap_or_default();
        PathScanResult::new(self.binary.clone(), failures)
    }
}

struct EventCounter(Arc<AtomicUsize>);

impl<S> Layer<S> for EventCounter
where
    S: Subscriber,
{
    /// Count events emitted while a dependency query is in scope.
    fn on_event(&self, _event: &tracing::Event<'_>, _context: Context<'_, S>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

/// Verify successful dependency lookup returns diagnostics without tracing them.
#[test]
fn dependency_query_returns_path_diagnostics_without_emitting_events() {
    let event_count = Arc::new(AtomicUsize::new(0));
    let subscriber = tracing_subscriber::registry().with(EventCounter(Arc::clone(&event_count)));
    let locator = FixedBinaryLocator {
        binary: Some(PathBuf::from("/fixture/dylint-link")),
        failure: Some(PathScanFailureCategory::DirectoryOpen),
    };
    let executor = StubExecutor::new(vec![
        cargo_dylint_check().expect("cargo-dylint version should resolve"),
        dylint_link_install_list_check().expect("dylint-link version should resolve"),
    ]);

    let check = tracing::subscriber::with_default(subscriber, || {
        check_dylint_tools_with_locator(&executor, &locator)
            .expect("a later PATH match should establish availability")
    });

    assert_eq!(event_count.load(Ordering::Relaxed), 0);
    assert_eq!(check.path_scan_failures.len(), 1);
    assert_eq!(
        check.path_scan_failures[0].category,
        PathScanFailureCategory::DirectoryOpen
    );
    assert!(check.status.all_installed());
    executor.assert_finished();
}

/// Verify unresolved PATH failures make availability queries fail.
#[rstest::rstest]
#[case::path(PathScanFailureCategory::DirectoryOpen, "directory_open")]
#[case::metadata(PathScanFailureCategory::Metadata, "metadata")]
fn unresolved_path_failures_are_errors(
    #[case] category: PathScanFailureCategory,
    #[case] expected_category: &str,
) {
    let locator = FixedBinaryLocator {
        binary: None,
        failure: Some(category),
    };
    let executor = StubExecutor::new(vec![
        cargo_dylint_check().expect("cargo-dylint version should resolve"),
    ]);

    let error = check_dylint_tools_with_locator(&executor, &locator)
        .expect_err("a failed PATH query must not report a missing tool");

    assert!(error.to_string().contains(expected_category));
    executor.assert_finished();
}

/// Verify command spawn errors remain visible to the dependency caller.
#[test]
fn command_spawn_errors_are_propagated() {
    let locator = FixedBinaryLocator {
        binary: None,
        failure: None,
    };
    let executor = StubExecutor::new(vec![ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Err(crate::error::InstallerError::Io(std::io::Error::other(
            "cargo could not be started",
        ))),
    }]);

    let error = check_dylint_tools_with_locator(&executor, &locator)
        .expect_err("a command spawn error must not look like a missing tool");

    assert!(error.to_string().contains("cargo could not be started"));
    executor.assert_finished();
}

/// Verify operational command failures are not treated as missing tools.
#[test]
fn command_failures_are_propagated() {
    let locator = FixedBinaryLocator {
        binary: None,
        failure: None,
    };
    let executor = StubExecutor::new(vec![ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Ok(failure_output(
            "cargo-dylint failed to read its configuration",
        )),
    }]);

    let error = check_dylint_tools_with_locator(&executor, &locator)
        .expect_err("an operational command failure must not look like a missing tool");

    assert!(
        error
            .to_string()
            .contains("failed to read its configuration")
    );
    executor.assert_finished();
}

/// Verify Cargo's explicit missing-subcommand response establishes absence.
#[test]
fn missing_cargo_subcommand_is_reported_as_absent() {
    let locator = FixedBinaryLocator {
        binary: None,
        failure: None,
    };
    let executor = StubExecutor::new(vec![ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Ok(failure_output("error: no such command: `dylint`")),
    }]);

    let check = check_dylint_tools_with_locator(&executor, &locator)
        .expect("Cargo's explicit missing-subcommand result establishes absence");

    assert_eq!(
        check.status,
        crate::deps::DylintToolStatus {
            cargo_dylint: false,
            dylint_link: false,
        }
    );
    executor.assert_finished();
}

/// Verify failed registry reads remain visible to the dependency caller.
#[test]
fn failed_registry_command_is_propagated() {
    let locator = FixedBinaryLocator {
        binary: Some(PathBuf::from("/fixture/dylint-link")),
        failure: None,
    };
    let executor = StubExecutor::new(vec![
        cargo_dylint_check().expect("cargo-dylint version should resolve"),
        ExpectedCall {
            cmd: "cargo",
            args: vec!["install", "--list"],
            result: Ok(failure_output("registry failure")),
        },
    ]);

    let error = check_dylint_tools_with_locator(&executor, &locator)
        .expect_err("a failed registry command must not look like an absent package");

    assert!(error.to_string().contains("registry failure"));
    executor.assert_finished();
}
