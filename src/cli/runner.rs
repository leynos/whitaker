//! Executes routed commands and forwards legacy Dylint invocations.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
};

use ortho_config::{Localizer, is_display_request};

use super::{
    Cli, Command,
    dispatch::{Class, TokenTag, classify},
    exit::Outcome,
    parser::is_common_cluster,
};
use crate::ports::{CliError, InstallRequest, ListRequest, Services};

/// Separate output sinks passed through a root command invocation.
pub struct CommandStreams<'a> {
    stdout: &'a mut dyn Write,
    stderr: &'a mut dyn Write,
}

impl<'a> CommandStreams<'a> {
    /// Creates a stream pair for command output and diagnostics.
    pub fn new(stdout: &'a mut dyn Write, stderr: &'a mut dyn Write) -> Self {
        Self { stdout, stderr }
    }
}

/// Routes and executes the argument vector, preserving the legacy Dylint path.
///
/// # Examples
///
/// ```
/// use std::io;
///
/// use ortho_config::NoOpLocalizer;
/// use whitaker::{adapters::SystemServices, cli};
///
/// let services = SystemServices::new();
/// let ports = services.ports();
/// let mut stdout = io::sink();
/// let mut stderr = io::sink();
/// let outcome = cli::run_from(
///     ["whitaker", "--help"],
///     &ports,
///     &NoOpLocalizer::new(),
///     &mut cli::CommandStreams::new(&mut stdout, &mut stderr),
/// );
/// assert_eq!(cli::exit::exit_code_for(&outcome), 0);
/// ```
pub fn run_from<I, T>(
    arguments: I,
    services: &Services<'_>,
    localizer: &dyn Localizer,
    streams: &mut CommandStreams<'_>,
) -> Outcome
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let raw: Vec<OsString> = arguments.into_iter().map(Into::into).collect();
    let route = classify(&tokenize_process_args(&raw));
    if route == Class::Forward {
        return forward_arguments(&raw, services, streams.stderr);
    }
    match Cli::try_parse_localized_from(raw, localizer) {
        Ok(cli) => run_command(&cli, services, streams),
        Err(error) if is_display_request(&error) => print_clap_error(&error, Outcome::Display),
        Err(error) => print_clap_error(&error, Outcome::UsageError),
    }
}

/// Dispatches a parsed root command to its installer or inventory port.
fn run_command(cli: &Cli, services: &Services<'_>, streams: &mut CommandStreams<'_>) -> Outcome {
    match &cli.command {
        Command::Install(parsed_arguments) => {
            run_install(cli.install_arguments(parsed_arguments), services, streams)
        }
        Command::Ls(arguments) => run_list(arguments, services, streams),
    }
}

/// Invokes the installer port and maps its result to a process outcome.
fn run_install(
    arguments: whitaker_installer::cli::InstallArgs,
    services: &Services<'_>,
    streams: &mut CommandStreams<'_>,
) -> Outcome {
    let result = services.install.install(
        &InstallRequest { arguments },
        streams.stdout,
        streams.stderr,
    );
    outcome_for_result(result, streams.stderr)
}

/// Collects and renders installed-lint inventory through the inventory port.
fn run_list(
    arguments: &whitaker_installer::cli::ListArgs,
    services: &Services<'_>,
    streams: &mut CommandStreams<'_>,
) -> Outcome {
    let result = services
        .inventory
        .list(&ListRequest {
            arguments: arguments.clone(),
        })
        .and_then(|inventory| write_inventory(streams.stdout, arguments.json, &inventory));
    outcome_for_result(result, streams.stderr)
}

/// Maps successful work or a CLI error to its command outcome.
fn outcome_for_result<T>(result: Result<T, CliError>, stderr: &mut dyn Write) -> Outcome {
    match result {
        Ok(_) => Outcome::Success,
        Err(error) => print_cli_error(&error, stderr),
    }
}

/// Writes the selected human-readable or JSON inventory representation.
fn write_inventory(
    stdout: &mut dyn Write,
    as_json: bool,
    inventory: &crate::ports::StagedInventory,
) -> Result<(), CliError> {
    let output = if as_json {
        whitaker_installer::list_output::format_json(
            &inventory.installed,
            inventory.active_toolchain.as_deref(),
        )
    } else {
        whitaker_installer::list_output::format_human(
            &inventory.installed,
            inventory.active_toolchain.as_deref(),
        )
    };
    writeln!(stdout, "{output}").map_err(|source| CliError::Output { source })
}

/// Writes an operational CLI error and returns its failure outcome.
fn print_cli_error(error: &CliError, stderr: &mut dyn Write) -> Outcome {
    writeln!(stderr, "{error}").unwrap_or(());
    Outcome::OperationalFailure
}

/// Prints a clap diagnostic through clap's own output streams.
fn print_clap_error(error: &clap::Error, outcome: Outcome) -> Outcome {
    match error.print() {
        Ok(()) => outcome,
        Err(_) => Outcome::OperationalFailure,
    }
}

/// Forwards legacy lint arguments to cargo-dylint and preserves its status.
fn forward_arguments(raw: &[OsString], services: &Services<'_>, stderr: &mut dyn Write) -> Outcome {
    let arguments: Vec<OsString> = raw.iter().skip(1).cloned().collect();
    match services.lint_runner.forward(&arguments) {
        Ok(code) => Outcome::Forwarded(code),
        Err(error) => print_cli_error(&error, stderr),
    }
}

/// Classifies every process argument after argv[0] for route selection.
fn tokenize_process_args(arguments: &[OsString]) -> Vec<TokenTag> {
    arguments
        .iter()
        .skip(1)
        .map(|argument| token_tag(argument))
        .collect()
}

/// Classifies one token; bare `help` is a root display request.
fn token_tag(argument: &OsStr) -> TokenTag {
    match argument.to_str() {
        Some("help" | "-h" | "--help") => TokenTag::Help,
        Some("install") => TokenTag::Install,
        Some("ls") => TokenTag::Ls,
        Some("-V" | "--version") => TokenTag::Version,
        Some("--") => TokenTag::DoubleDash,
        Some("-q" | "--quiet" | "-v" | "--verbose" | "--verbosity") => TokenTag::GlobalFlag,
        Some(value) if is_common_cluster(value) => TokenTag::GlobalFlag,
        Some(value) if value.starts_with('-') => TokenTag::DylintFlag,
        Some(_) | None => TokenTag::Other,
    }
}
