//! Tests for the no-source-fallback policy and its contradictions.

use super::NO_SOURCE_FALLBACK_ENV;
use crate::cli::{InstallArgs, LintSelectionFlags};
use whitaker_common::test_support::env_test_guard;

/// The rule, as one table.
///
/// Each entry is the environment value, the `--no-source-fallback` flag, and
/// the verdict the pair must produce. A caller who exported the variable meant
/// something by it, so an unrecognized value enables the rule rather than
/// silently disabling a protection; empty, `0` and `false` are the three shapes
/// that conventionally mean "off".
///
/// Both tests below read this table. Two copies of it would drift, and the
/// point of the pair is that the pure form and the process-reading wrapper
/// agree on the same cases.
const POLICY_CASES: &[(Option<&str>, bool, bool)] = &[
    (None, false, false),
    (None, true, true),
    (Some(""), false, false),
    (Some("0"), false, false),
    (Some("false"), false, false),
    (Some("FALSE"), false, false),
    (Some(" false "), false, false),
    (Some("1"), false, true),
    (Some("true"), false, true),
    (Some("yes"), false, true),
    (Some("false"), true, true),
];

/// Build arguments carrying only the flag under test.
fn install_args(no_source_fallback: bool) -> InstallArgs {
    InstallArgs {
        no_source_fallback,
        ..InstallArgs::default()
    }
}

/// Every case in one test, deliberately.
///
/// The cases mutate one process-wide environment variable. As separate
/// `rstest` cases they can run on separate threads of the same process
/// under `cargo test`, where one case's `with_var` would be visible to
/// another and the failure would be intermittent. Iterating here keeps
/// the mutation serial whatever the runner does.
#[test]
fn the_environment_and_the_flag_agree_on_the_rule() {
    // One process-wide variable, so the guard rather than the loop: a
    // sibling test running concurrently would otherwise observe this one's
    // value and fail intermittently.
    let _guard = env_test_guard();
    for (environment, flag, expected) in POLICY_CASES {
        temp_env::with_var(NO_SOURCE_FALLBACK_ENV, *environment, || {
            assert_eq!(
                install_args(*flag).forbids_source_fallback(),
                *expected,
                "environment {environment:?} with flag {flag} should give {expected}"
            );
        });
    }
}

/// A present but unreadable value must enable the rule.
///
/// Reading bytes that cannot be inspected as "off" would disable the
/// protection in exactly the case where nothing can be said about intent.
#[cfg(unix)]
#[test]
fn a_non_unicode_environment_value_enables_the_rule() {
    // One process-wide variable, so the guard rather than the loop: a
    // sibling test running concurrently would otherwise observe this one's
    // value and fail intermittently.
    let _guard = env_test_guard();
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let invalid = OsString::from_vec(vec![0x66, 0x61, 0x6c, 0xff]);
    temp_env::with_var(NO_SOURCE_FALLBACK_ENV, Some(invalid), || {
        assert!(install_args(false).forbids_source_fallback());
    });
}

/// The environment can enable the rule where clap cannot see it.
///
/// clap rejects the contradictory flags, so only the environment route
/// needs asserting here; without the post-parse check the run would be
/// told to build from source and forbidden from doing so.
#[test]
fn the_environment_conflicts_with_a_source_only_option() {
    // One process-wide variable, so the guard rather than the loop: a
    // sibling test running concurrently would otherwise observe this one's
    // value and fail intermittently.
    let _guard = env_test_guard();
    let cases: &[(&str, InstallArgs)] = &[
        (
            "--build-only",
            InstallArgs {
                is_build_only: true,
                ..InstallArgs::default()
            },
        ),
        (
            "--experimental",
            InstallArgs {
                lint_selection: super::super::LintSelectionFlags {
                    experimental: true,
                    ..super::super::LintSelectionFlags::default()
