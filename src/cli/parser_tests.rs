//! Tests for localized parsing and the shared root flags.

use ortho_config::NoOpLocalizer;

use super::{Cli, Command, is_common_cluster};

#[test]
fn common_flags_are_accepted_before_and_after_subcommands() {
    let before = Cli::try_parse_localized_from(
        ["whitaker", "-q", "install", "--dry-run"],
        &NoOpLocalizer::new(),
    )
    .expect("quiet before install should parse");
    assert!(before.quiet, "quiet should be stored on the root command");
    let after = Cli::try_parse_localized_from(
        ["whitaker", "install", "-vv", "--dry-run"],
        &NoOpLocalizer::new(),
    )
    .expect("repeated verbosity after install should parse");
    assert_eq!(after.verbosity, 2, "repeated verbosity should be counted");
}

#[test]
fn root_flags_are_applied_to_the_shared_installer_request() {
    let cli = Cli::try_parse_localized_from(
        ["whitaker", "install", "--quiet", "--dry-run"],
        &NoOpLocalizer::new(),
    )
    .expect("install command should parse");
    let Command::Install(parsed) = &cli.command else {
        panic!("command should be install");
    };
    let effective = cli.install_arguments(parsed);
    assert!(
        effective.quiet,
        "shared quiet option should reach installer"
    );
    assert!(
        effective.execution.dry_run,
        "installer option should remain set"
    );
}

#[test]
fn attached_value_options_are_not_classified_as_global_clusters() {
    assert!(is_common_cluster("-vq"));
    assert!(!is_common_cluster("-vj2"));
    assert!(!is_common_cluster("-qj2"));
}

#[test]
fn a_target_directory_is_refused_with_the_reason() {
    let error = Cli::try_parse_localized_from(
        ["whitaker", "check", "--target-dir", "out"],
        &NoOpLocalizer::new(),
    )
    .expect_err("`--target-dir` cannot be honoured");

    assert!(
        error
            .to_string()
            .contains("Dylint injects its own target directory"),
        "the refusal must explain why, got: {error}"
    );
}

#[cfg(unix)]
#[test]
fn invalid_utf8_arguments_remain_parseable_as_os_strings() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let invalid = OsString::from_vec(vec![0xff]);
    let parsed = Cli::try_parse_localized_from(
        [
            OsString::from("whitaker"),
            OsString::from("install"),
            invalid,
        ],
        &NoOpLocalizer::new(),
    );
    assert!(
        parsed.is_err(),
        "invalid install argument should be rejected"
    );
}
