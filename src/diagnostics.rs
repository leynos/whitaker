//! Process-wide diagnostics for the root `whitaker` command.
//!
//! The shared installer library emits `tracing` events but deliberately never
//! installs a subscriber: a library that did so would contend with its host
//! application's own exporter. This module is the root binary's installation,
//! and it mirrors the installer binary's policy so `-q` and `-v` mean the same
//! thing whichever Whitaker command a user runs.

use std::io::Stderr;

use tracing::Level;
use tracing_subscriber::{
    filter::LevelFilter,
    fmt::{
        SubscriberBuilder,
        format::{DefaultFields, Format},
    },
};

/// The root command's diagnostic policy, resolved from its global flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiagnosticSettings {
    /// Suppress every event below `ERROR`.
    pub quiet: bool,
    /// How many times `--verbose` was given; any non-zero count enables debug.
    pub verbosity: u8,
}

impl DiagnosticSettings {
    /// The highest level this policy lets through.
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing::Level;
    /// use whitaker::diagnostics::DiagnosticSettings;
    ///
    /// assert_eq!(DiagnosticSettings::default().level(), Level::WARN);
    /// assert_eq!(
    ///     DiagnosticSettings {
    ///         quiet: false,
    ///         verbosity: 1
    ///     }
    ///     .level(),
    ///     Level::DEBUG
    /// );
    /// assert_eq!(
    ///     DiagnosticSettings {
    ///         quiet: true,
    ///         verbosity: 3
    ///     }
    ///     .level(),
    ///     Level::ERROR
    /// );
    /// ```
    #[must_use]
    pub const fn level(self) -> Level {
        if self.quiet {
            Level::ERROR
        } else if self.verbosity > 0 {
            Level::DEBUG
        } else {
            Level::WARN
        }
    }
}

/// Describe a failure to register the root diagnostic subscriber.
#[derive(Debug, thiserror::Error)]
#[error("failed to initialize diagnostic tracing: {source}")]
pub struct DiagnosticInitializationError {
    /// The underlying global subscriber registration failure.
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
}

/// Install the process-wide subscriber under `settings`.
///
/// # Errors
///
/// Returns an error when a global subscriber is already installed.
pub fn configure(settings: DiagnosticSettings) -> Result<(), DiagnosticInitializationError> {
    formatter(settings)
        .try_init()
        .map_err(|source| DiagnosticInitializationError { source })
}

/// Build the subscriber without installing it.
///
/// Construction is separate so the tests can capture events through the
/// production filter and ANSI policy instead of a lookalike.
fn formatter(
    settings: DiagnosticSettings,
) -> SubscriberBuilder<DefaultFields, Format, LevelFilter, fn() -> Stderr> {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(settings.level())
        .with_writer(std::io::stderr as fn() -> Stderr)
}

#[cfg(test)]
mod tests {
    //! Exercise the production formatter without touching the global state.

    use std::io::{Read, Seek};

    use rstest::rstest;

    use super::*;

    /// Re-run this test binary with `probe_env` set.
    ///
    /// A global subscriber can be installed only once per process, so each
    /// probe needs a process of its own.
    fn run_probe(probe_env: &str, test_name: &str) -> std::process::Output {
        std::process::Command::new(
            std::env::current_exe().expect("test executable must be discoverable"),
        )
        .args(["--exact", test_name, "--nocapture"])
        .env(probe_env, "1")
        .output()
        .expect("isolated tracing probe must run")
    }

    /// The child half of the stderr probe: install the real subscriber.
    fn emit_through_installed_subscriber() {
        configure(DiagnosticSettings::default()).expect("child probe subscriber must initialize");
        tracing::warn!("root stderr diagnostic");
    }

    /// Verify the installed subscriber writes to stderr, never stdout.
    #[test]
    fn configure_writes_to_stderr() {
        const PROBE_ENV: &str = "WHITAKER_ROOT_TRACING_STDERR_PROBE";
        if std::env::var_os(PROBE_ENV).is_some() {
            emit_through_installed_subscriber();
            return;
        }
        let output = run_probe(PROBE_ENV, "diagnostics::tests::configure_writes_to_stderr");
        assert!(
            output.status.success(),
            "tracing probe must succeed: {output:?}"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");
        let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
        assert!(stderr.contains("root stderr diagnostic"));
        assert!(!stdout.contains("root stderr diagnostic"));
        assert!(!stderr.contains('\u{1b}'), "stderr must omit ANSI escapes");
    }

    /// The child half of the failure probe: occupy the global slot first.
    fn reject_a_second_subscriber() {
        tracing::subscriber::set_global_default(tracing_subscriber::registry())
            .expect("probe subscriber must be installed first");
        let error = configure(DiagnosticSettings::default())
            .expect_err("a second global subscriber must be rejected");
        assert!(
            error
                .to_string()
                .contains("failed to initialize diagnostic tracing")
        );
    }

    /// Verify a pre-installed global subscriber becomes a reported error.
    #[test]
    fn configure_reports_a_second_subscriber() {
        const PROBE_ENV: &str = "WHITAKER_ROOT_TRACING_INITIALIZATION_FAILURE_PROBE";
        if std::env::var_os(PROBE_ENV).is_some() {
            reject_a_second_subscriber();
            return;
        }
        let output = run_probe(
            PROBE_ENV,
            "diagnostics::tests::configure_reports_a_second_subscriber",
        );
        assert!(
            output.status.success(),
            "tracing failure probe must report the error: {output:?}"
        );
    }

    /// Emit an `ERROR` event, so the filter's cut-off is observable.
    fn emit_error() {
        tracing::error!("error diagnostic");
    }

    /// Emit a `WARN` event, so the filter's cut-off is observable.
    fn emit_warn() {
        tracing::warn!("warning diagnostic");
    }

    /// Emit an `INFO` event, so the filter's cut-off is observable.
    fn emit_info() {
        tracing::info!("info diagnostic");
    }

    /// Emit a `DEBUG` event, so the filter's cut-off is observable.
    fn emit_debug() {
        tracing::debug!("debug diagnostic");
    }

    /// Emit a `TRACE` event, so the filter's cut-off is observable.
    fn emit_trace() {
        tracing::trace!("trace diagnostic");
    }

    /// Emit one event at every level, so the filter's cut-off is observable.
    fn emit_every_level() {
        emit_error();
        emit_warn();
        emit_info();
        emit_debug();
        emit_trace();
    }

    /// Read the formatted messages back from a capture file.
    fn captured_messages(mut capture: std::fs::File) -> Vec<String> {
        capture.rewind().expect("capture file must rewind");
        let mut output = String::new();
        capture
            .read_to_string(&mut output)
            .expect("formatted diagnostics must be UTF-8");
        assert!(
            !output.contains('\u{1b}'),
            "diagnostics must omit ANSI escapes"
        );
        output
            .lines()
            .map(|line| {
                line.split_once(": ")
                    .expect("event must include its formatted message")
                    .1
                    .to_owned()
            })
            .collect()
    }

    /// Verify each policy through emitted, formatted events.
    #[rstest]
    #[case::quiet(true, 0, &["error diagnostic"])]
    #[case::default(false, 0, &["error diagnostic", "warning diagnostic"])]
    #[case::verbose(false, 1, &["error diagnostic", "warning diagnostic", "info diagnostic", "debug diagnostic"])]
    #[case::repeated_verbose(false, 2, &["error diagnostic", "warning diagnostic", "info diagnostic", "debug diagnostic"])]
    #[case::quiet_overrides_verbose(true, 2, &["error diagnostic"])]
    fn diagnostics_respect_verbosity_without_ansi(
        #[case] quiet: bool,
        #[case] verbosity: u8,
        #[case] expected_messages: &[&str],
    ) {
        let settings = DiagnosticSettings { quiet, verbosity };
        let capture = tempfile::tempfile().expect("diagnostic capture file must be created");
        let subscriber = formatter(settings)
            .with_writer(capture.try_clone().expect("capture file must be cloned"))
            .finish();

        tracing::subscriber::with_default(subscriber, emit_every_level);

        let messages = captured_messages(capture);
        assert_eq!(messages, expected_messages);
    }
}
