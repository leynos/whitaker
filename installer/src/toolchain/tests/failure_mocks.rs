//! Failure-oriented test helpers for toolchain installation tests.

use super::*;
use crate::toolchain::tests::test_helpers::{
    ToolchainInstallExpectation, expect_rustc_version, expect_toolchain_install,
    matches_multi_component_add, output_with_status, output_with_stderr,
};

/// Describes the type of installation failure being tested.
#[derive(Debug, Clone, Copy)]
pub(super) enum InstallFailure {
    ToolchainInstall,
    ComponentAdd,
    CraneliftComponentAdd,
    ToolchainUnusableAfterInstall,
}

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
            stderr: Some("network down"),
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
        .returning(|_, _| Ok(output_with_stderr(1, "component failed")));
}

fn setup_component_add_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
) {
    setup_component_add_failure_mocks_inner(runner, seq, channel, &[]);
}

fn setup_cranelift_component_add_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
) {
    setup_component_add_failure_mocks_inner(runner, seq, channel, &[CRANELIFT_COMPONENT]);
}

fn setup_toolchain_unusable_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
) {
    let expected_components = REQUIRED_COMPONENTS;
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
        .withf(matches_multi_component_add(channel, expected_components))
        .times(1)
        .in_sequence(seq)
        .returning(|_, _| Ok(output_with_status(0)));
    expect_rustc_version(runner, seq, channel, 1);
}

pub(super) fn setup_failure_mocks(
    runner: &mut MockCommandRunner,
    seq: &mut mockall::Sequence,
    channel: &str,
    failure: InstallFailure,
) {
    match failure {
        InstallFailure::ToolchainInstall => {
            setup_toolchain_install_failure_mocks(runner, seq, channel);
        }
        InstallFailure::ComponentAdd => {
            setup_component_add_failure_mocks(runner, seq, channel);
        }
        InstallFailure::CraneliftComponentAdd => {
            setup_cranelift_component_add_failure_mocks(runner, seq, channel);
        }
        InstallFailure::ToolchainUnusableAfterInstall => {
            setup_toolchain_unusable_failure_mocks(runner, seq, channel);
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

fn is_cranelift_component_install_failed(err: &InstallerError, channel: &str) -> bool {
    let InstallerError::ToolchainComponentInstallFailed {
        toolchain,
        components,
        message,
    } = err
    else {
        return false;
    };
    if toolchain != channel {
        return false;
    }
    if !components.contains(CRANELIFT_COMPONENT) {
        return false;
    }
    message.contains("component failed")
}

pub(super) fn assert_failure_error(err: InstallerError, channel: &str, failure: InstallFailure) {
    match failure {
        InstallFailure::ToolchainInstall => assert_error_matches(
            &err,
            &format!("ToolchainInstallFailed for {channel}"),
            |e| {
                matches!(
                    e,
                    InstallerError::ToolchainInstallFailed { toolchain, message }
                        if toolchain == channel && message.contains("network down")
                )
            },
        ),
        InstallFailure::ComponentAdd => assert_error_matches(
            &err,
            &format!("ToolchainComponentInstallFailed for {channel}"),
            |e| {
                matches!(
                    e,
                    InstallerError::ToolchainComponentInstallFailed {
                        toolchain,
                        message,
                        ..
                    } if toolchain == channel && message.contains("component failed")
                )
            },
        ),
        InstallFailure::CraneliftComponentAdd => assert_error_matches(
            &err,
            &format!("ToolchainComponentInstallFailed with cranelift for {channel}"),
            |e| is_cranelift_component_install_failed(e, channel),
        ),
        InstallFailure::ToolchainUnusableAfterInstall => {
            assert_error_matches(&err, &format!("ToolchainNotInstalled for {channel}"), |e| {
                matches!(
                    e,
                    InstallerError::ToolchainNotInstalled { toolchain }
                        if toolchain == channel
                )
            })
        }
    }
}
