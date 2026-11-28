//! Property tests for the argument vector `whitaker check` assembles.
//!
//! The property is stated end to end, through the path a real command line
//! takes: shared-option extraction, clap parsing, and assembly. Assembling a
//! vector the parser never produces would stay green while the parser dropped
//! or reordered the user's arguments, so the property drives the parser too.
//!
//! Two things keep it honest. The generator classifies its cases, and the run
//! fails unless each shape fills its share, so a generator that stopped
//! producing an inner `--` cannot pass the property vacuously. The fixed table
//! below states the interesting command lines literally, including the
//! separator cases a generator may reach only rarely.

use std::{cell::RefCell, ffi::OsString};

use ortho_config::NoOpLocalizer;
use proptest::{prelude::*, test_runner::TestRunner};
use rstest::rstest;
use whitaker::cli::{
    Cli,
    Command,
    check::{
        CheckArgs,
        argv::{DylintOption, assemble_dylint_args},
    },
};

/// The number of cases the property runs.
const CASES: u32 = 4_096;

/// Where a shared option sits relative to the `check` subcommand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// No shared option was given.
    Absent,
    /// The option precedes the subcommand.
    Before,
    /// The option follows the subcommand, before the separator.
    After,
    /// The option appears on both sides.
    Both,
}

/// One generated command line, before it is turned into arguments.
#[derive(Debug, Clone)]
struct Case {
    /// The shared option, drawn from the closed set the root accepts.
    flag: OsString,
    /// Where that option sits.
    placement: Placement,
    /// Whether `--no-install` was given.
    no_install: bool,
    /// The arguments forwarded after the separator.
    user: Vec<OsString>,
}

/// Build the command line the property parses for `case`.
fn command_line(case: &Case) -> Vec<OsString> {
    let mut argv = vec![OsString::from("whitaker")];
    if matches!(case.placement, Placement::Before | Placement::Both) {
        argv.push(case.flag.clone());
    }
    argv.push(OsString::from("check"));
    if matches!(case.placement, Placement::After | Placement::Both) {
        argv.push(case.flag.clone());
    }
    if case.no_install {
        argv.push(OsString::from("--no-install"));
    }
    argv.push(OsString::from("--"));
    argv.extend(case.user.iter().cloned());
    argv
}

/// Parse `case`, or fail the property with the parser's own message.
fn parse_case(case: &Case) -> Result<(CheckArgs, bool, u8), TestCaseError> {
    let cli = Cli::try_parse_localized_from(command_line(case), &NoOpLocalizer::new()).map_err(
        |error| TestCaseError::fail(format!("the generated command line must parse: {error}")),
    )?;
    let Cli {
        quiet,
        verbosity,
        command,
    } = cli;
    let Command::Check(check) = command else {
        return Err(TestCaseError::fail(
            "the generated command line must select `check`".to_owned(),
        ));
    };
    Ok((check, quiet, verbosity))
}

/// The shared settings the option and its placements should produce.
fn expected_settings(case: &Case) -> (bool, u8) {
    let placements: u8 = match case.placement {
        Placement::Absent => 0,
        Placement::Before | Placement::After => 1,
        Placement::Both => 2,
    };
    // Each `-v` counts once, and a second placement of `-vv` counts again.
    let per_flag: u8 = match case.flag.to_str() {
        Some("-v") => 1,
        Some("-vv") => 2,
        _ => 0,
    };
    (case.flag == "-q" && placements > 0, per_flag * placements)
}

/// Build the expected Whitaker prefix from the literal command line.
///
/// Written out rather than derived from [`DylintOption::SET`] so the property
/// compares the assembler against an independent statement of the contract:
/// reversing or reordering the set would change the actual vector while the
/// expected one stayed put.
fn whitaker_prefix() -> Vec<OsString> {
    ["--lib=whitaker_suite", "--no-metadata", "--"]
        .map(OsString::from)
        .to_vec()
}

/// Check one case end to end, returning the property's verdict.
fn check_case(case: &Case) -> Result<(), TestCaseError> {
    let (check, quiet, verbosity) = parse_case(case)?;
    let (want_quiet, want_verbosity) = expected_settings(case);

    prop_assert_eq!(
        (quiet, verbosity),
        (want_quiet, want_verbosity),
        "shared options must be read from anywhere before the separator"
    );
    prop_assert_eq!(
        check.no_install,
        case.no_install,
        "`--no-install` must reach the parsed arguments"
    );

    let assembled = assemble_dylint_args(&check.cargo_args);
    let prefix = whitaker_prefix();
    prop_assert_eq!(
        assembled.len(),
        prefix.len() + case.user.len(),
        "assembly forwarded {} arguments for {} user arguments",
        assembled.len(),
        case.user.len(),
    );
    let (head, tail) = assembled.split_at(prefix.len());
    prop_assert_eq!(
        head,
        prefix.as_slice(),
        "Whitaker's options and separator must come first"
    );
    prop_assert_eq!(
        tail,
        case.user.as_slice(),
        "the user's arguments must survive the round trip byte for byte"
    );

    // The first separator is Whitaker's own, whatever the user forwarded.
    let first_separator = assembled
        .iter()
        .position(|argument| argument == "--")
        .ok_or_else(|| TestCaseError::fail("the assembled vector holds no separator".to_owned()))?;
    prop_assert_eq!(
        first_separator,
        prefix.len() - 1,
        "no user argument may precede Whitaker's separator"
    );
    Ok(())
}

/// Count the shapes the property actually reached.
#[derive(Debug, Default)]
struct Coverage {
    /// How many cases were observed.
    cases: u32,
    /// How many forwarded an empty vector.
    empty: u32,
    /// How many forwarded an inner `--`.
    separator: u32,
    /// How many forwarded a first argument starting with a dash.
    leading_dash: u32,
}

impl Coverage {
    /// Record one observed user vector.
    fn observe(&mut self, user: &[OsString]) {
        self.cases += 1;
        if user.is_empty() {
            self.empty += 1;
        }
        if user.iter().any(|argument| argument == "--") {
            self.separator += 1;
        }
        if user
            .first()
            .is_some_and(|argument| argument.to_string_lossy().starts_with('-'))
        {
            self.leading_dash += 1;
        }
    }

    /// Require the shape named by `label` to fill at least a tenth of the run.
    fn require_share(&self, label: &str, count: u32) {
        assert!(
            count * 10 >= self.cases,
            "at least a tenth of the {} cases must cover {label}, but {count} did",
            self.cases,
        );
    }
}

/// Draw the shared option from the closed set the root accepts.
fn flags() -> impl Strategy<Value = OsString> {
    prop::sample::select(vec![
        OsString::from("-q"),
        OsString::from("-v"),
        OsString::from("-vv"),
    ])
}

/// Draw where the shared option sits.
fn placements() -> impl Strategy<Value = Placement> {
    prop::sample::select(vec![
        Placement::Absent,
        Placement::Before,
        Placement::After,
        Placement::Both,
    ])
}

/// Draw a token that is not valid UTF-8, where the platform can express one.
#[cfg(unix)]
fn non_utf8_token() -> impl Strategy<Value = OsString> {
    use std::os::unix::ffi::OsStringExt as _;

    (0x80_u8..=0xff).prop_map(|byte| OsString::from_vec(vec![b'f', byte]))
}

/// Stand in for [`non_utf8_token`] where no such argument exists.
#[cfg(not(unix))]
fn non_utf8_token() -> impl Strategy<Value = OsString> { Just(OsString::from("fallback")) }

/// Draw one forwarded argument from the shapes a real command line carries.
fn tokens() -> impl Strategy<Value = OsString> {
    prop_oneof![
        Just(OsString::from("--")),
        Just(OsString::from("-p")),
        Just(OsString::from("-q")),
        Just(OsString::from("--help")),
        Just(OsString::from("--all-targets")),
        Just(OsString::from("--message-format=json")),
        Just(OsString::new()),
        Just(OsString::from("two words")),
        Just(OsString::from("naïve")),
        Just(OsString::from("-x")),
        non_utf8_token(),
    ]
}

/// Draw a forwarded vector, with empty vectors frequent enough to count.
///
/// An even length in `0..=16` would put the empty case below a tenth of the
/// run, so the empty vector is drawn as its own branch rather than left to the
/// length distribution.
fn user_arguments() -> impl Strategy<Value = Vec<OsString>> {
    prop_oneof![Just(0_usize), 1_usize..=16]
        .prop_flat_map(|length| prop::collection::vec(tokens(), length))
}

/// Draw a whole command line.
fn cases() -> impl Strategy<Value = Case> {
    (flags(), placements(), any::<bool>(), user_arguments()).prop_map(
        |(flag, placement, no_install, user)| Case {
            flag,
            placement,
            no_install,
            user,
        },
    )
}

/// Require every forwarded argument, and every shared option, to survive.
///
/// The coverage checks are part of the test rather than a note beside it: a
/// generator that stopped producing an inner `--` would leave the property
/// green while never reaching the case a split-at-separator bug hides in.
#[test]
fn every_forwarded_argument_reaches_the_assembled_vector_unchanged() {
    let mut runner = TestRunner::new(ProptestConfig::with_cases(CASES));
    let coverage = RefCell::new(Coverage::default());

    runner
        .run(&cases(), |case| {
            coverage.borrow_mut().observe(&case.user);
            check_case(&case)
        })
        .expect("the end-to-end argument property must hold");

    let reached = coverage.into_inner();
    assert_eq!(
        reached.cases, CASES,
        "every generated case must be observed exactly once"
    );
    reached.require_share("an empty user vector", reached.empty);
    reached.require_share("an inner `--`", reached.separator);
    reached.require_share(
        "a first argument starting with a dash",
        reached.leading_dash,
    );
}

/// Assemble each named input shape and compare against the literal vector.
///
/// The generator may rarely produce a literal `--`, so this table states the
/// separator case directly: assembly must keep a user separator rather than
/// dropping or reordering it.
#[rstest]
#[case::empty(&[], &["--lib=whitaker_suite", "--no-metadata", "--"])]
#[case::plain(
    &["-p", "whitaker-common"],
    &["--lib=whitaker_suite", "--no-metadata", "--", "-p", "whitaker-common"]
)]
#[case::separator(
    &["-p", "--", "-q"],
    &["--lib=whitaker_suite", "--no-metadata", "--", "-p", "--", "-q"]
)]
fn assembly_covers_every_named_input_shape(#[case] user: &[&str], #[case] expected: &[&str]) {
    let forwarded: Vec<OsString> = user.iter().map(OsString::from).collect();
    let wanted: Vec<OsString> = expected.iter().map(OsString::from).collect();

    assert_eq!(
        assemble_dylint_args(&forwarded),
        wanted,
        "assembly must keep Whitaker's prefix first and the user's tokens in order"
    );
}

/// Read a shared option before the separator and forward it after it.
///
/// These are the four command lines whose meaning a reader is most likely to
/// get wrong: a `-q` on either side of `check` is Whitaker's, and a `-q` after
/// `--` belongs to Cargo. The property reaches them only by chance.
#[rstest]
#[case::quiet_after(&["whitaker", "check", "-q"], true, 0, &[])]
#[case::quiet_before(&["whitaker", "-q", "check"], true, 0, &[])]
#[case::quiet_forwarded(&["whitaker", "check", "--", "-q"], false, 0, &["-q"])]
#[case::help_forwarded(&["whitaker", "check", "--", "--help"], false, 0, &["--help"])]
fn a_shared_option_is_read_before_the_separator_and_forwarded_after_it(
    #[case] argv: &[&str],
    #[case] quiet: bool,
    #[case] verbosity: u8,
    #[case] forwarded: &[&str],
) {
    let cli = Cli::try_parse_localized_from(argv.to_vec(), &NoOpLocalizer::new())
        .expect("the fixed command line is valid");
    let Command::Check(check) = cli.command else {
        panic!("the fixed command line must select `check`");
    };
    let wanted: Vec<OsString> = forwarded.iter().map(OsString::from).collect();

    assert_eq!(
        (cli.quiet, cli.verbosity),
        (quiet, verbosity),
        "a shared option must be read only before the separator"
    );
    assert_eq!(
        check.cargo_args, wanted,
        "everything after the separator belongs to Cargo, whatever it looks like"
    );
}

/// Require each Whitaker-owned option to render as one non-separator argument.
///
/// That is what makes the prefix's first `--` unambiguously Whitaker's own.
#[test]
fn no_whitaker_option_renders_as_a_separator() {
    for option in DylintOption::SET {
        let token = option.token();

        assert!(
            !token.is_empty(),
            "{option:?} must render as a non-empty argument"
        );
        assert!(
            token != "--",
            "{option:?} must never be mistaken for the separator"
        );
        assert!(
            !token.to_string_lossy().contains(char::is_whitespace),
            "{option:?} must render as exactly one argument"
        );
    }
}
