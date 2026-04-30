//! Failure-oriented test helpers for toolchain installation tests.

use super::*;
use crate::toolchain::tests::test_helpers::{
    ToolchainInstallExpectation, expect_rustc_version, expect_toolchain_install,
    matches_multi_component_add, output_with_status, output_with_stderr,
};

/// Describes the type of installation failure being tested.
#[derive(Debug, Clone, Copy)]
pub(super) enum InstallFailure {
    /// `rustup toolchain install` exits unsuccessfully before components are
    /// added.
    ToolchainInstall,
    /// `rustup component add` exits unsuccessfully for the required components,
    /// plus any additional components requested by the test.
    ComponentAdd,
    /// `rustup toolchain install` succeeds, but the installed toolchain remains
    /// unusable when `rustc --version` is checked afterwards.
    ToolchainUnusableAfterInstall,
}

/// Bundles the failure mode with any extra components requested for the test.
#[derive(Debug, Clone, Copy)]
pub(super) struct FailureSetup<'a> {
    /// The failure branch that the mocks should exercise.
    pub(super) failure: InstallFailure,
    /// Extra `rustup` components expected alongside the required components.
    pub(super) additional_components: &'a [&'a str],
}

/// A typed toolchain channel identifier (e.g. `"nightly-2025-09-18"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ToolchainChannel<'a>(pub(super) &'a str);

impl<'a> ToolchainChannel<'a> {
    /// Returns the inner channel string slice (e.g. `"nightly-2025-09-18"`).
    pub(super) fn as_str(self) -> &'a str {
        self.0
    }
}

/// The exact stderr string emitted by the mock when a toolchain installation
/// fails. Used as the `message` payload in [`InstallerError::ToolchainInstallFailed`]
/// and matched with strict equality in [`assert_failure_error`] to prevent
/// wording-change regressions from going undetected.
pub(super) const TOOLCHAIN_INSTALL_FAILURE_MESSAGE: &str = "network down";
/// The exact stderr string emitted by the mock when a component installation
/// fails. Matched with strict equality (not a substring) in
/// `is_component_install_failed` so that any superstring variant is
/// correctly rejected. The unit test
/// `component_failure_match_rejects_superstring_messages` guards this
/// invariant.
pub(super) const COMPONENT_INSTALL_FAILURE_MESSAGE: &str = "component failed";

fn setup_toolchain_install_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
) {
    expect_rustc_version(runner, seq, channel, 1);
    expect_toolchain_install(
        runner,
        seq,
        ToolchainInstallExpectation {
            channel,
            exit_code: 1,
            stderr: Some(TOOLCHAIN_INSTALL_FAILURE_MESSAGE),
        },
    );
}

fn setup_component_add_failure_mocks_inner(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
    extra_components: &[&str],
) {
    let components = [REQUIRED_COMPONENTS, extra_components].concat();
    expect_rustc_version(runner, seq, channel, 0);
    runner
        .expect_run()
        .withf(matches_multi_component_add(channel, &components))
        .times(1)
        .in_sequence(seq)
        .returning(|_, _| Ok(output_with_stderr(1, COMPONENT_INSTALL_FAILURE_MESSAGE)));
}

fn setup_toolchain_unusable_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
    additional_components: &[&str],
) {
    let components = [REQUIRED_COMPONENTS, additional_components].concat();
    expect_rustc_version(runner, seq, channel, 1);
    expect_toolchain_install(
        runner,
        seq,
        ToolchainInstallExpectation {
            channel,
            exit_code: 0,
            stderr: None,
        },
    );
    runner
        .expect_run()
        .withf(matches_multi_component_add(channel, &components))
        .times(1)
        .in_sequence(seq)
        .returning(|_, _| Ok(output_with_status(0)));
    expect_rustc_version(runner, seq, channel, 1);
}

/// Configures `runner` and `seq` to simulate the failure described by `setup`
/// for the given toolchain `channel`.
///
/// Sets up mock expectations in sequence so that `ensure_installed_with` will
/// trigger the error variant corresponding to `setup.failure`. Any extra
/// components in `setup.additional_components` are included in the expected
/// `rustup component add` invocation where applicable.
///
/// # Arguments
///
/// - `runner` - Mock command runner that receives the expected `rustup` and
///   `rustc` calls.
/// - `seq` - Mockall sequence enforcing the order of expected commands.
/// - `channel` - Toolchain channel being installed or checked.
/// - `setup` - Failure mode and additional components to model.
pub(super) fn setup_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: ToolchainChannel<'_>,
    setup: FailureSetup<'_>,
) {
    let channel = channel.as_str();
    match setup.failure {
        InstallFailure::ToolchainInstall => {
            setup_toolchain_install_failure_mocks(runner, seq, channel);
        }
        InstallFailure::ComponentAdd => {
            setup_component_add_failure_mocks_inner(
                runner,
                seq,
                channel,
                setup.additional_components,
            );
        }
        InstallFailure::ToolchainUnusableAfterInstall => {
            setup_toolchain_unusable_failure_mocks(
                runner,
                seq,
                channel,
                setup.additional_components,
            );
        }
    }
}

fn failure_summary(setup: FailureSetup<'_>) -> String {
    match setup.failure {
        InstallFailure::ToolchainInstall => "toolchain installation failure".to_owned(),
        InstallFailure::ComponentAdd => format!(
            "component installation failure for {}",
            expected_components(setup.additional_components)
        ),
        InstallFailure::ToolchainUnusableAfterInstall => {
            "toolchain unusable after installation".to_owned()
        }
    }
}

/// Asserts that `err` satisfies `predicate`, printing `description` on failure.
fn assert_error_matches<F>(err: &InstallerError, description: &str, predicate: F)
where
    F: FnOnce(&InstallerError) -> bool,
{
    assert!(predicate(err), "expected {description}, got {err:?}");
}

fn expected_components(additional_components: &[&str]) -> String {
    [REQUIRED_COMPONENTS, additional_components]
        .concat()
        .join(", ")
}

fn is_component_install_failed(
    err: &InstallerError,
    channel: &str,
    additional_components: &[&str],
) -> bool {
    let expected = expected_components(additional_components);
    let InstallerError::ToolchainComponentInstallFailed {
        toolchain,
        components,
        message,
        ..
    } = err
    else {
        return false;
    };
    if toolchain != channel {
        return false;
    }
    components == &expected && message == COMPONENT_INSTALL_FAILURE_MESSAGE
}

/// Asserts that `err` is the `InstallerError` variant expected for `setup.failure`
/// on toolchain `channel`.
///
/// For `ComponentAdd`, also verifies the reported component list includes the
/// required components plus any in `setup.additional_components`.
///
/// # Arguments
///
/// - `err` - Installer error returned by the code under test.
/// - `channel` - Toolchain channel expected in the error payload.
/// - `setup` - Failure mode and additional components that define the expected
///   error shape.
///
/// # Panics
///
/// Panics with a descriptive message if the error variant or its fields do not
/// match expectations.
pub(super) fn assert_failure_error(
    err: InstallerError,
    channel: ToolchainChannel<'_>,
    setup: FailureSetup<'_>,
) {
    let channel = channel.as_str();
    let failure = failure_summary(setup);
    match setup.failure {
        InstallFailure::ToolchainInstall => assert_error_matches(
            &err,
            &format!("ToolchainInstallFailed for channel {channel} while exercising {failure}"),
            |e| {
                matches!(
                    e,
                    InstallerError::ToolchainInstallFailed { toolchain, message }
                        if toolchain == channel && message == TOOLCHAIN_INSTALL_FAILURE_MESSAGE
                )
            },
        ),
        InstallFailure::ComponentAdd => assert_error_matches(
            &err,
            &format!(
                "ToolchainComponentInstallFailed for channel {channel} while exercising {failure}"
            ),
            |e| is_component_install_failed(e, channel, setup.additional_components),
        ),
        InstallFailure::ToolchainUnusableAfterInstall => assert_error_matches(
            &err,
            &format!("ToolchainNotInstalled for channel {channel} while exercising {failure}"),
            |e| {
                matches!(
                    e,
                    InstallerError::ToolchainNotInstalled { toolchain }
                        if toolchain == channel
                )
            },
        ),
    }
}

/// Unit tests for failure-oriented toolchain installation helpers.
#[cfg(test)]
mod tests;
