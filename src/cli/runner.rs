//! Executes routed commands and forwards legacy Dylint invocations.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
};

use ortho_config::{Localizer, is_display_request};

use super::{
    Cli,
    Command,
    check::{self, CheckRequest, exit::CheckOutcome},
    dispatch::{Class, Subcommand, TokenTag, classify},
    exit::Outcome,
    parser::is_common_cluster,
};
use crate::{
    adapters::telemetry::{self, DiagnosticStream},
    ports::{CliError, InstallRequest, ListRequest, Services},
};

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
        Ok(cli) => run_command(&cli, services, localizer, streams),
        Err(error) if is_display_request(&error) => print_clap_error(
            &error,
            ClapOutcomes {
                written: Outcome::Display,
                lost: lost_diagnostic(route),
            },
            localizer,
            streams,
        ),
        Err(error) => print_clap_error(
            &error,
            ClapOutcomes {
                written: Outcome::UsageError,
                lost: lost_diagnostic(route),
            },
            localizer,
            streams,
        ),
    }
}

/// Dispatches a parsed root command to the port that implements it.
fn run_command(
    cli: &Cli,
    services: &Services<'_>,
    localizer: &dyn Localizer,
    streams: &mut CommandStreams<'_>,
) -> Outcome {
    match &cli.command {
        Command::Install(parsed_arguments) => {
            run_install(cli.install_arguments(parsed_arguments), services, streams)
        }
        Command::Ls(arguments) => run_list(arguments, services, streams),
        // The root flags travel with the request so an implicit repair
        // inherits them: `whitaker -q check` must not print an installer's
        // progress.
        Command::Check(arguments) => check::run_check_command(
            CheckRequest::new(arguments, cli.diagnostic_settings()),
            localizer,
            services.check,
            streams.stderr,
        ),
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
///
/// A failed write cannot change the outcome, because the command has already
/// failed. It is still reported through `tracing` and counted as a failed
/// diagnostic so a broken stream is visible rather than silently discarded.
fn print_cli_error(error: &CliError, stderr: &mut dyn Write) -> Outcome {
    let written = writeln!(stderr, "{error}");
    if let Err(source) = &written {
        tracing::error!(%source, "failed to write the CLI error to stderr");
    }
    telemetry::record_diagnostic_write(DiagnosticStream::Stderr, written.is_ok());
    Outcome::OperationalFailure
}

/// The outcome to report when a clap diagnostic never reached its stream.
///
/// `check` owns its own status contract, so a lost diagnostic is its
/// `exec.output` failure rather than the generic operational one.
const fn lost_diagnostic(route: Class) -> Outcome {
    match route {
        Class::Subcommand(Subcommand::Check) => Outcome::Check(CheckOutcome::OutputFailed),
        Class::Subcommand(Subcommand::Install | Subcommand::Ls)
        | Class::Forward
        | Class::Display => Outcome::OperationalFailure,
    }
}

/// What a clap diagnostic's write resolves to, by whether it reached a stream.
///
/// The two outcomes travel together so a broken pipe is never reported as the
/// usage error the lost text was describing.
#[derive(Debug, Clone, Copy)]
struct ClapOutcomes {
    /// Reported when the text reached its stream.
    written: Outcome,
    /// Reported when it did not.
    lost: Outcome,
}

/// Writes a clap diagnostic to the stream clap selects for it.
fn print_clap_error(
    error: &clap::Error,
    outcomes: ClapOutcomes,
    localizer: &dyn Localizer,
    streams: &mut CommandStreams<'_>,
) -> Outcome {
    let stream = if error.use_stderr() {
        DiagnosticStream::Stderr
    } else {
        DiagnosticStream::Stdout
    };
    let written = match stream {
        DiagnosticStream::Stderr => deliver(streams.stderr, error),
        DiagnosticStream::Stdout => deliver(streams.stdout, error),
    };
    telemetry::record_diagnostic_write(stream, written.is_ok());
    if written.is_ok() {
        return outcomes.written;
    }
    // The failed outcome may still have a reason key to report, and its own
    // stream may be the one that still works. `check` is the only route whose
    // failure contract requires that; a generic operational failure has no
    // key to print.
    if let Outcome::Check(failure) = outcomes.lost {
        check::report_outcome(failure, localizer, streams.stderr);
    }
    outcomes.lost
}

/// Write a rendered diagnostic and flush the stream it went to.
///
/// A buffered stream can accept every byte and still fail to deliver it, so the
/// flush belongs to the same fallible operation: a help text that never reached
/// the caller must not be reported as a successful display request.
fn deliver(stream: &mut dyn Write, error: &clap::Error) -> std::io::Result<()> {
    write!(stream, "{}", error.render())?;
    stream.flush()
}

/// Forwards legacy lint arguments to cargo-dylint and preserves its status.
fn forward_arguments(raw: &[OsString], services: &Services<'_>, stderr: &mut dyn Write) -> Outcome {
    let arguments: Vec<OsString> = raw.iter().skip(1).cloned().collect();
    match services.lint_runner.forward(&arguments, stderr) {
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
        Some("check") => TokenTag::Check,
        Some("-V" | "--version") => TokenTag::Version,
        Some("--") => TokenTag::DoubleDash,
        Some("-q" | "--quiet" | "-v" | "--verbose" | "--verbosity") => TokenTag::GlobalFlag,
        Some(value) if is_common_cluster(value) => TokenTag::GlobalFlag,
        Some(value) if value.starts_with('-') => TokenTag::DylintFlag,
        Some(_) | None => TokenTag::Other,
    }
}

#[cfg(test)]
mod tests {
    //! Tests for routing parser diagnostics through the supplied streams.

    use std::io::{self, Write};

    use super::{CheckOutcome, CommandStreams, Outcome, print_cli_error, run_from};
    use crate::{adapters::SystemServices, ports::CliError};

    /// A stream that refuses every write, standing in for a closed stderr.
    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("stderr is closed"))
        }

        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    /// A buffered stream that accepts bytes it can never deliver.
    struct UnflushableWriter;

    impl Write for UnflushableWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> { Ok(buffer.len()) }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("the buffer could not be flushed"))
        }
    }

    #[test]
    fn a_failing_diagnostic_stream_still_reports_operational_failure() {
        let error = CliError::Inventory {
            message: "the inventory is unavailable".to_owned(),
        };
        let mut stderr = FailingWriter;

        assert_eq!(
            print_cli_error(&error, &mut stderr),
            Outcome::OperationalFailure,
            "the command has already failed, so a lost diagnostic cannot change the outcome"
        );
    }

    /// Report a help text the buffer never delivered as lost output.
    #[test]
    fn a_help_text_that_cannot_be_flushed_is_reported_as_lost() {
        let services = SystemServices::new();
        let ports = services.ports();
        let mut stdout = UnflushableWriter;
        let mut stderr = Vec::new();

        let outcome = run_from(
            ["whitaker", "check", "--help"],
            &ports,
            &ortho_config::NoOpLocalizer::new(),
            &mut CommandStreams::new(&mut stdout, &mut stderr),
        );

        assert_eq!(
            outcome,
            Outcome::Check(CheckOutcome::OutputFailed),
            "a help text the buffer never delivered must report the lost output"
        );
        assert!(
            !stderr.is_empty(),
            "the lost outcome must still be reported on the stream that works"
        );
    }

    /// Send each clap diagnostic to the stream clap selected for it.
    #[test]
    fn clap_diagnostics_use_the_configured_streams() {
        let services = SystemServices::new();
        let ports = services.ports();

        for (arguments, expected, uses_stderr) in [
            (&["whitaker", "--help"][..], Outcome::Display, false),
            (
                &["whitaker", "install", "--invalid-option"][..],
                Outcome::UsageError,
                true,
            ),
        ] {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let outcome = run_from(
                arguments.iter().copied(),
                &ports,
                &ortho_config::NoOpLocalizer::new(),
                &mut CommandStreams::new(&mut stdout, &mut stderr),
            );

            assert_eq!(outcome, expected);
            assert_eq!(stdout.is_empty(), uses_stderr);
            assert_eq!(stderr.is_empty(), !uses_stderr);
        }
    }
}
