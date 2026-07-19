//! Tests for dependency binary helper fixtures and expected call builders.

use crate::test_utils::dependency_binary_helpers::{
    ExpectedCallConfig, dependency_version, expected_calls, repository_verification_call,
};

#[test]
fn repository_verification_call_returns_probe_for_cargo_dylint() {
    let call = repository_verification_call("cargo-dylint", false);
    let call = match call {
        Some(call) => call,
        None => panic!("cargo-dylint should use a verification probe"),
    };

    assert_eq!(call.cmd, "cargo");
    assert_eq!(call.args, vec!["dylint", "--version"]);
    let output = call.result.expect("verification probe should succeed");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(
        stdout.contains(dependency_version("cargo-dylint")),
        "expected stdout to report the manifest version, got {stdout:?}"
    );
}

#[rstest::rstest]
#[case::successful_verification(false)]
#[case::failed_verification(true)]
fn repository_verification_call_skips_executor_for_dylint_link(#[case] verification_fails: bool) {
    // Repository installs of dylint-link are verified by probing the
    // extracted binary directly, so no executor call is expected either way.
    assert!(repository_verification_call("dylint-link", verification_fails).is_none());
}

#[test]
fn expected_calls_include_repository_probe_for_cargo_dylint() {
    let calls = expected_calls(
        "cargo-dylint",
        ExpectedCallConfig {
            is_binstall_available: false,
            has_repository_context: true,
            is_repository_asset_missing: false,
            should_verify_repository_install: true,
            is_repository_verification_failing: false,
            cargo_binstall_failure: None,
            cargo_install_failure: None,
        },
    );

    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].cmd, "cargo");
    assert_eq!(calls[1].args, vec!["dylint", "--version"]);
}

#[test]
fn expected_calls_omit_executor_verification_for_dylint_link() {
    let calls = expected_calls(
        "dylint-link",
        ExpectedCallConfig {
            is_binstall_available: false,
            has_repository_context: true,
            is_repository_asset_missing: false,
            should_verify_repository_install: true,
            is_repository_verification_failing: false,
            cargo_binstall_failure: None,
            cargo_install_failure: None,
        },
    );

    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].cmd, "cargo");
    assert_eq!(calls[0].args, vec!["binstall", "--version"]);
}
