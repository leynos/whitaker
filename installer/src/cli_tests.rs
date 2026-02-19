//! Tests for installer CLI parsing and default behaviours.

use super::*;
use rstest::rstest;

#[test]
fn cli_parses_defaults() {
    let cli = Cli::parse_from(["whitaker-installer"]);
    assert!(cli.command.is_none());
    assert!(cli.install.target_dir.is_none());
    assert!(cli.install.lint.is_empty());
    assert!(!cli.install.individual_lints);
    assert!(!cli.install.experimental);
    assert!(!cli.install.dry_run);
    assert_eq!(cli.install.verbosity, 0);
    assert!(!cli.install.quiet);
    assert!(!cli.install.skip_deps);
    assert!(!cli.install.skip_wrapper);
    assert!(!cli.install.no_update);
    assert!(!cli.install.is_build_only);
}

#[test]
fn cli_parses_target_dir() {
    let cli = Cli::parse_from(["whitaker-installer", "-t", "/tmp/dylint"]);
    assert_eq!(
        cli.install.target_dir,
        Some(Utf8PathBuf::from("/tmp/dylint"))
    );
}

#[test]
fn cli_parses_multiple_lints() {
    let cli = Cli::parse_from([
        "whitaker-installer",
        "-l",
        "module_max_lines",
        "-l",
        "no_expect_outside_tests",
    ]);
    assert_eq!(cli.install.lint.len(), 2);
}

#[test]
fn cli_parses_list_subcommand() {
    let cli = Cli::parse_from(["whitaker-installer", "list"]);
    assert!(matches!(cli.command, Some(Command::List(_))));
}

#[test]
fn cli_parses_list_with_json() {
    let cli = Cli::parse_from(["whitaker-installer", "list", "--json"]);
    match cli.command {
        Some(Command::List(args)) => assert!(args.json),
        _ => panic!("expected List command"),
    }
}

#[test]
fn cli_parses_list_with_target_dir() {
    let cli = Cli::parse_from(["whitaker-installer", "list", "-t", "/custom/path"]);
    match cli.command {
        Some(Command::List(args)) => {
            assert_eq!(args.target_dir, Some(Utf8PathBuf::from("/custom/path")));
        }
        _ => panic!("expected List command"),
    }
}

#[test]
fn cli_parses_install_subcommand() {
    let cli = Cli::parse_from(["whitaker-installer", "install"]);
    assert!(matches!(cli.command, Some(Command::Install(_))));
}

#[test]
fn cli_parses_install_with_args() {
    let cli = Cli::parse_from([
        "whitaker-installer",
        "install",
        "--experimental",
        "-l",
        "module_max_lines",
    ]);
    match cli.command {
        Some(Command::Install(args)) => {
            assert!(args.experimental);
            assert_eq!(args.lint, vec!["module_max_lines"]);
        }
        _ => panic!("expected Install command"),
    }
}

#[test]
fn should_attempt_prebuilt_true_for_default_configuration() {
    let args = InstallArgs::default();
    let requested = vec![CrateName::from("whitaker_suite")];
    assert!(args.should_attempt_prebuilt(&requested));
}

#[test]
fn should_attempt_prebuilt_false_when_build_only() {
    let args = InstallArgs {
        is_build_only: true,
        ..InstallArgs::default()
    };
    let requested = vec![CrateName::from("whitaker_suite")];
    assert!(!args.should_attempt_prebuilt(&requested));
}

#[test]
fn should_attempt_prebuilt_false_when_experimental_flag_enabled() {
    let args = InstallArgs {
        experimental: true,
        ..InstallArgs::default()
    };
    let requested = vec![CrateName::from("whitaker_suite")];
    assert!(!args.should_attempt_prebuilt(&requested));
}

#[test]
fn should_attempt_prebuilt_true_for_stable_bumpy_road_requests() {
    let args = InstallArgs::default();
    let requested = vec![CrateName::from("bumpy_road_function")];
    assert!(args.should_attempt_prebuilt(&requested));
}

/// Parameterised tests for boolean CLI flags (backwards compatibility).
#[rstest]
#[case::individual_lints(&["whitaker-installer", "--individual-lints"], |cli: &Cli| cli.install.individual_lints)]
#[case::experimental(&["whitaker-installer", "--experimental"], |cli: &Cli| cli.install.experimental)]
#[case::dry_run(&["whitaker-installer", "--dry-run"], |cli: &Cli| cli.install.dry_run)]
#[case::verbose(&["whitaker-installer", "-v"], |cli: &Cli| cli.install.verbosity > 0)]
#[case::quiet(&["whitaker-installer", "-q"], |cli: &Cli| cli.install.quiet)]
#[case::skip_deps(&["whitaker-installer", "--skip-deps"], |cli: &Cli| cli.install.skip_deps)]
#[case::skip_wrapper(&["whitaker-installer", "--skip-wrapper"], |cli: &Cli| cli.install.skip_wrapper)]
#[case::no_update(&["whitaker-installer", "--no-update"], |cli: &Cli| cli.install.no_update)]
#[case::build_only(&["whitaker-installer", "--build-only"], |cli: &Cli| cli.install.is_build_only)]
fn cli_parses_boolean_flags(#[case] args: &[&str], #[case] check: fn(&Cli) -> bool) {
    let cli = Cli::parse_from(args);
    assert!(check(&cli));
}

/// Parameterised tests for repeatable verbosity flags.
#[rstest]
#[case::double_short(&["whitaker-installer", "-vv"], 2)]
#[case::triple_short(&["whitaker-installer", "-vvv"], 3)]
#[case::double_long(&["whitaker-installer", "--verbose", "--verbose"], 2)]
#[case::double_alias(&["whitaker-installer", "--verbosity", "--verbosity"], 2)]
fn cli_parses_repeatable_verbosity_flags(#[case] args: &[&str], #[case] expected: u8) {
    let cli = Cli::parse_from(args);
    assert_eq!(cli.install.verbosity, expected);
}

#[rstest]
#[case::individual_lints_with_lint(&["whitaker-installer", "--individual-lints", "--lint", "module_max_lines"])]
#[case::verbose_with_quiet(&["whitaker-installer", "--verbose", "--quiet"])]
fn cli_rejects_conflicting_flags(#[case] args: &[&str]) {
    Cli::try_parse_from(args).expect_err("expected clap to reject conflicting flags");
}

/// Verify the Default impl produces a valid baseline configuration.
#[test]
fn install_args_default_is_valid() {
    let args = InstallArgs::default();
    assert!(!args.individual_lints);
    assert!(!args.experimental);
    assert!(!args.skip_deps);
}

#[test]
fn list_args_default_is_valid() {
    let args = ListArgs::default();
    assert!(!args.json);
    assert!(args.target_dir.is_none());
}

#[test]
fn install_args_returns_flattened_when_no_subcommand() {
    let cli = Cli::parse_from(["whitaker-installer", "--experimental"]);
    let args = cli.install_args();
    assert!(args.experimental);
}

#[test]
fn install_args_returns_subcommand_args_when_present() {
    let cli = Cli::parse_from(["whitaker-installer", "install", "--dry-run"]);
    let args = cli.install_args();
    assert!(args.dry_run);
}
