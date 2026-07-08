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
    assert!(!cli.install.cranelift);
    assert!(!cli.install.dry_run);
    assert_eq!(cli.install.verbosity, 0);
    assert!(!cli.install.quiet);
    assert!(!cli.install.skip_deps);
    assert!(!cli.install.skip_wrapper);
    assert!(!cli.install.no_update);
    assert!(!cli.install.is_build_only);
    assert!(cli.install.git_ref.is_none());
}

fn cli_parses_ref_flag_bare() {
    let cli = Cli::parse_from(["whitaker-installer", "--ref", "v0.2.5"]);
    assert_eq!(cli.install.git_ref.as_deref(), Some("v0.2.5"));
}

fn cli_parses_ref_flag_under_install_subcommand() {
    let cli = Cli::parse_from(["whitaker-installer", "install", "--ref", "1a2b3c4d"]);
    match cli.command {
        Some(Command::Install(args)) => {
            assert_eq!(args.git_ref.as_deref(), Some("1a2b3c4d"));
        }
        _ => panic!("expected Install command"),
    }
}
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
fn should_attempt_prebuilt_false_when_the_suite_is_pinned() {
    // The rolling release carries the tip and nothing else, so a pinned
    // install can only ever be served by a source build. Attempting the
    // download first would spend a request to learn what the pin already says.
    let args = InstallArgs {
        suite_version: Some("v0.2.7".try_into().expect("valid reference")),
        ..InstallArgs::default()
    };
    let requested = vec![CrateName::from("whitaker_suite")];
    assert!(!args.should_attempt_prebuilt(&requested));
}

#[test]
fn should_attempt_prebuilt_true_when_the_suite_is_not_pinned() {
    // The counterpart, so the skip cannot quietly become unconditional.
    let args = InstallArgs {
        suite_version: None,
        ..InstallArgs::default()
    };
    let requested = vec![CrateName::from("whitaker_suite")];
    assert!(args.should_attempt_prebuilt(&requested));
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

/// Parameterized tests for boolean CLI flags (backwards compatibility).
#[rstest]
#[case::individual_lints(&["whitaker-installer", "--individual-lints"], |cli: &Cli| cli.install.individual_lints)]
#[case::experimental(&["whitaker-installer", "--experimental"], |cli: &Cli| cli.install.experimental)]
#[case::cranelift(&["whitaker-installer", "--cranelift"], |cli: &Cli| cli.install.cranelift)]
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

/// The suite pin is the one flag whose value reaches a `git` command line,
/// so both the accepted spellings and the refusals are held here.
#[rstest]
#[case::tag(&["whitaker-installer", "--suite-version", "v0.2.7"], "v0.2.7")]
#[case::equals_form(&["whitaker-installer", "--suite-version=v0.2.7"], "v0.2.7")]
#[case::alias(&["whitaker-installer", "--suite-ref", "main"], "main")]
#[case::commit(&["whitaker-installer", "--suite-version", "2bc0c3f"], "2bc0c3f")]
fn cli_parses_the_suite_pin(#[case] args: &[&str], #[case] expected: &str) {
    let cli = Cli::parse_from(args);

    let reference = cli.install.suite_version.expect("pin should have parsed");
    assert_eq!(reference.as_str(), expected);
}

#[rstest]
fn cli_leaves_the_suite_unpinned_by_default() {
    // The default is the branch tip, which keeps the prebuilt fast path
    // reachable. Pinning is opt-in and costs a source build.
    let cli = Cli::parse_from(["whitaker-installer"]);

    assert!(cli.install.suite_version.is_none());
}

#[rstest]
#[case::option_injection(&["whitaker-installer", "--suite-version=--upload-pack=touch /tmp/x"])]
#[case::traversal(&["whitaker-installer", "--suite-version=v1..v2"])]
#[case::reflog(&["whitaker-installer", "--suite-version=main@{yesterday}"])]
#[case::space(&["whitaker-installer", "--suite-version=my branch"])]
#[case::empty(&["whitaker-installer", "--suite-version="])]
fn cli_refuses_a_reference_git_would_not_take(#[case] args: &[&str]) {
    // Refused at the boundary rather than passed to git and explained after a
    // clone has already been paid for.
    let result = Cli::try_parse_from(args);

    assert!(result.is_err(), "{args:?} should have been rejected");
}

/// Parameterized tests for repeatable verbosity flags.
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
    assert!(!args.cranelift);
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
