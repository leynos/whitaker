//! Tests for dependency binary helper fixtures and expected call builders.

use crate::test_utils::{
    ExpectedCall,
    dependency_binary_helpers::{
        ExpectedCallConfig,
        RepositoryVerification,
        dependency_version,
        expected_calls,
        repository_verification_call,
    },
};

// Assertions live in these helpers because the tests below return `Result` to
// propagate fixture errors, and `panic_in_result_fn` denies asserting inside
// such a function.

/// Assert the recorded call probes `cargo dylint --version`.
fn assert_cargo_dylint_probe(call: ExpectedCall, expected_version: &str) {
    assert_eq!(call.cmd, "cargo", "the verification probe runs cargo");
    assert_eq!(
        call.args,
        vec!["dylint", "--version"],
        "the probe asks for the dylint version"
    );
    let output = call.result.expect("verification probe should succeed");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(
        stdout.contains(expected_version),
        "expected stdout to report the manifest version, got {stdout:?}"
    );
}

/// Assert the recorded call probes `cargo binstall --version` alone.
fn assert_binstall_probe_recorded(calls: &[ExpectedCall]) {
    assert_eq!(calls.len(), 1, "only the binstall probe should be recorded");
    let probe = calls.first().expect("binstall probe should be recorded");
    assert_eq!(probe.cmd, "cargo", "the binstall probe runs cargo");
    assert_eq!(
        probe.args,
        vec!["binstall", "--version"],
        "the probe asks for the binstall version"
    );
}

/// Assert the repository probe and its verification call were both recorded.
fn assert_repository_probe_recorded(calls: &[ExpectedCall]) {
    assert_eq!(
        calls.len(),
        2,
        "the repository probe should be followed by verification"
    );
    let verification = calls.get(1).expect("verification probe should be recorded");
    assert_eq!(verification.cmd, "cargo", "verification runs cargo");
    assert_eq!(
        verification.args,
        vec!["dylint", "--version"],
        "verification asks for the dylint version"
    );
}

#[test]
fn repository_verification_call_returns_probe_for_cargo_dylint() -> Result<(), String> {
    let call = repository_verification_call("cargo-dylint", false)?
        .expect("cargo-dylint should use a verification probe");

    assert_cargo_dylint_probe(call, dependency_version("cargo-dylint")?);
    Ok(())
}

#[rstest::rstest]
#[case::successful_verification(false)]
#[case::failed_verification(true)]
fn repository_verification_call_skips_executor_for_dylint_link(#[case] verification_fails: bool) {
    // Repository installs of dylint-link are verified by probing the
    // extracted binary directly, so no executor call is expected either way.
    let call = repository_verification_call("dylint-link", verification_fails)
        .expect("the verification probe lookup should succeed");
    assert!(
        call.is_none(),
        "dylint-link is verified without an executor call"
    );
}

/// Return the repository-install configuration shared by the cases below.
///
/// Both cases install from a repository whose asset is present and whose
/// version probe succeeds, so they differ only in the binary being installed.
fn repository_context_config() -> ExpectedCallConfig<'static> {
    ExpectedCallConfig {
        is_binstall_available: false,
        has_repository_context: true,
        is_repository_asset_missing: false,
        repository_verification: RepositoryVerification::Succeeds,
        cargo_binstall_failure: None,
        cargo_install_failure: None,
    }
}

#[test]
fn expected_calls_include_repository_probe_for_cargo_dylint() -> Result<(), String> {
    let calls = expected_calls("cargo-dylint", &repository_context_config())?;

    assert_repository_probe_recorded(&calls);
    Ok(())
}

#[test]
fn expected_calls_omit_executor_verification_for_dylint_link() -> Result<(), String> {
    let calls = expected_calls("dylint-link", &repository_context_config())?;

    assert_binstall_probe_recorded(&calls);
    Ok(())
}
