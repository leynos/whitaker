//! Short-option cluster cases where root and legacy parsers must agree.
//!
//! Clustered short flags such as `-vj2` and option values that look like flags
//! exercise clap's tokenizer rather than the option table, so the generated
//! parity property does not reach them.

use std::ffi::OsString;

use whitaker::cli::{Cli, Command};
use whitaker_installer::cli::{Cli as InstallerCli, InstallArgs};

use super::{parse_legacy, parse_root};

#[derive(Clone, Copy)]
struct ExpectedCommonOptions {
    quiet: bool,
    verbosity: u8,
    jobs: Option<usize>,
}

struct ShortClusterCase {
    arguments: &'static [&'static str],
    expected: Option<ExpectedCommonOptions>,
}

impl ShortClusterCase {
    const fn accepting(
        arguments: &'static [&'static str],
        quiet: bool,
        verbosity: u8,
        jobs: Option<usize>,
    ) -> Self {
        Self {
            arguments,
            expected: Some(ExpectedCommonOptions {
                quiet,
                verbosity,
                jobs,
            }),
        }
    }

    const fn rejecting(arguments: &'static [&'static str]) -> Self {
        Self {
            arguments,
            expected: None,
        }
    }
}

#[test]
fn root_install_matches_legacy_for_parser_edge_cases() {
    let cases = [
        ShortClusterCase::accepting(&["-vj2"], false, 1, Some(2)),
        ShortClusterCase::accepting(&["-qj2"], true, 0, Some(2)),
        ShortClusterCase::accepting(&["-vvj2"], false, 2, Some(2)),
        ShortClusterCase::accepting(&["-j2", "-v"], false, 1, Some(2)),
        ShortClusterCase::accepting(&["-v", "-j2"], false, 1, Some(2)),
        ShortClusterCase::rejecting(&["-vj2", "-q"]),
        ShortClusterCase::rejecting(&["-v", "-qj2"]),
        ShortClusterCase::rejecting(&["-qj2", "-v"]),
        ShortClusterCase::rejecting(&["-q", "-vj2"]),
        ShortClusterCase::rejecting(&["-vqj2"]),
        ShortClusterCase::rejecting(&["-qvj2"]),
        ShortClusterCase::rejecting(&["--jobs", "-v", "2"]),
        ShortClusterCase::rejecting(&["--toolchain", "-v", "nightly-test"]),
        ShortClusterCase::rejecting(&["-q", "-q"]),
    ];

    for test_case in cases {
        verify_short_cluster_case(&test_case)
            .expect("root and legacy parsers should agree for each short-cluster case");
    }
}

fn verify_short_cluster_case(test_case: &ShortClusterCase) -> Result<(), String> {
    let parsed_arguments = test_case
        .arguments
        .iter()
        .copied()
        .map(OsString::from)
        .collect::<Vec<_>>();
    for explicit_subcommand in [false, true] {
        verify_short_cluster_case_for_form(test_case, &parsed_arguments, explicit_subcommand)?;
    }
    Ok(())
}

fn verify_short_cluster_case_for_form(
    test_case: &ShortClusterCase,
    parsed_arguments: &[OsString],
    explicit_subcommand: bool,
) -> Result<(), String> {
    let root_result = parse_root(parsed_arguments);
    let legacy_result = parse_legacy(parsed_arguments, explicit_subcommand);
    match test_case.expected {
        Some(_) => verify_accepting_short_cluster_case(
            test_case,
            root_result,
            legacy_result,
            explicit_subcommand,
        ),
        None => verify_rejecting_short_cluster_case(
            test_case,
            root_result,
            legacy_result,
            explicit_subcommand,
        ),
    }
}

fn verify_accepting_short_cluster_case(
    test_case: &ShortClusterCase,
    root_result: Result<Cli, clap::Error>,
    legacy_result: Result<InstallerCli, clap::Error>,
    explicit_subcommand: bool,
) -> Result<(), String> {
    let Some(expected_options) = test_case.expected else {
        return Err("accepting parser case must declare expected options".to_owned());
    };
    let root = root_result.map_err(|error| {
        format!(
            "root parser rejected valid arguments {:?}: {error}",
            test_case.arguments
        )
    })?;
    let legacy = legacy_result.map_err(|error| {
        format!(
            "legacy parser rejected valid arguments {:?} with \
             explicit_subcommand={explicit_subcommand}: {error}",
            test_case.arguments
        )
    })?;
    let Command::Install(parsed_root_arguments) = &root.command else {
        return Err(format!(
            "root parser produced a different command for {:?}",
            test_case.arguments
        ));
    };
    let root_arguments = root.install_arguments(parsed_root_arguments);
    let legacy_arguments = legacy.install_args();
    if root_arguments != *legacy_arguments {
        return Err(format!(
            "parser arguments differ for {:?} with explicit_subcommand={explicit_subcommand}",
            test_case.arguments
        ));
    }
    verify_expected_common_options(test_case.arguments, &root_arguments, expected_options)
}

fn verify_rejecting_short_cluster_case(
    test_case: &ShortClusterCase,
    root_result: Result<Cli, clap::Error>,
    legacy_result: Result<InstallerCli, clap::Error>,
    explicit_subcommand: bool,
) -> Result<(), String> {
    let Err(root_error) = root_result else {
        return Err(format!(
            "root parser accepted conflicting arguments {:?}",
            test_case.arguments
        ));
    };
    let Err(legacy_error) = legacy_result else {
        return Err(format!(
            "legacy parser accepted conflicting arguments {:?} with \
             explicit_subcommand={explicit_subcommand}",
            test_case.arguments
        ));
    };
    if root_error.kind() != legacy_error.kind()
        || root_error.exit_code() != legacy_error.exit_code()
    {
        return Err(format!(
            "parser rejection differs for {:?} with explicit_subcommand={explicit_subcommand}",
            test_case.arguments
        ));
    }
    Ok(())
}

fn verify_expected_common_options(
    arguments: &[&str],
    actual: &InstallArgs,
    expected: ExpectedCommonOptions,
) -> Result<(), String> {
    if actual.quiet != expected.quiet {
        return Err(format!("quiet flag differs for {arguments:?}"));
    }
    if actual.verbosity != expected.verbosity {
        return Err(format!("verbosity differs for {arguments:?}"));
    }
    if actual.jobs != expected.jobs {
        return Err(format!("job count differs for {arguments:?}"));
    }
    Ok(())
}
