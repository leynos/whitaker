//! Configure and test the installer CLI's diagnostic subscriber.

use std::io::Stderr;

use tracing::Level;
use tracing_subscriber::{
    filter::LevelFilter,
    fmt::{
        SubscriberBuilder,
        format::{DefaultFields, Format},
    },
};
use whitaker_installer::cli::InstallArgs;

/// Describe a CLI tracing subscriber registration failure.
#[derive(Debug, thiserror::Error)]
#[error("failed to initialize diagnostic tracing: {source}")]
pub(crate) struct TracingInitializationError {
    /// The underlying global subscriber registration failure.
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
}

/// Enable bounded stderr diagnostics according to install verbosity flags.
///
/// # Errors
///
/// Returns an error if another global subscriber has already been installed.
pub(crate) fn configure_tracing(
    args: &InstallArgs,
) -> std::result::Result<(), TracingInitializationError> {
    tracing_formatter(args)
        .try_init()
        .map_err(|source| TracingInitializationError { source })
}

/// Build the CLI-only formatter without installing a global subscriber.
///
/// Keep construction separate so tests can capture events using the production
/// filter and ANSI policy. Only CLI startup and this module's tests use it.
fn tracing_formatter(
    args: &InstallArgs,
) -> SubscriberBuilder<DefaultFields, Format, LevelFilter, fn() -> Stderr> {
    let max_level = if args.quiet {
        Level::ERROR
    } else if args.verbosity > 0 {
        Level::DEBUG
    } else {
        Level::WARN
    };
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(max_level)
        .with_writer(std::io::stderr as fn() -> Stderr)
}

#[cfg(test)]
mod tests {
    //! Exercise the production formatter without global subscriber mutation.

    use std::io::{Read, Seek};

    #[cfg(unix)]
    use camino::Utf8Path;
    #[cfg(unix)]
    use cap_std::{ambient_authority, fs_utf8::Dir};
    #[cfg(unix)]
    use clap::Parser;
    use rstest::rstest;
    #[cfg(unix)]
    use whitaker_installer::cli::Cli;

    use super::*;

    #[cfg(unix)]
    const PATH_SCAN_PROBE_ENV: &str = "WHITAKER_TRACING_PATH_SCAN_PROBE";

    /// Verify CLI flags filter the real PATH-scan diagnostic in a child process.
    #[cfg(unix)]
    #[test]
    fn cli_flags_filter_path_scan_diagnostics_on_stderr() {
        if std::env::var_os(PATH_SCAN_PROBE_ENV).is_some() {
            run_path_scan_probe();
            return;
        }

        let fixture = path_scan_fixture();
        for mode in ["default", "verbose", "quiet"] {
            let output = run_path_scan_child(mode, &fixture.path);
            assert_probe_status(mode, &output);
            assert_probe_diagnostics(mode, &output);
            assert_probe_redaction(&output, &fixture);
        }
    }

    /// A PATH fixture whose first entry is a file rather than a directory.
    #[cfg(unix)]
    struct PathScanFixture {
        /// Keeps the temporary directory alive for the fixture's lifetime.
        _directory: tempfile::TempDir,
        /// The joined PATH value handed to the child process.
        path: std::ffi::OsString,
        /// The unreadable first entry the diagnostic must not disclose.
        invalid_entry: std::path::PathBuf,
        /// The directory holding the fake linker.
        binary_directory: std::path::PathBuf,
    }

    /// Build a PATH with an invalid entry, a binary directory, and a fake
    /// `dylint-link`, keeping the temporary directory alive.
    #[cfg(unix)]
    fn path_scan_fixture() -> PathScanFixture {
        let fixture = tempfile::tempdir().expect("PATH fixture directory must be created");
        let invalid_entry = fixture.path().join("not-a-directory");
        let fixture_path =
            Utf8Path::from_path(fixture.path()).expect("temporary directory path must be UTF-8");
        let fixture_dir = Dir::open_ambient_dir(fixture_path, ambient_authority())
            .expect("temporary directory must open as a capability");
        fixture_dir
            .write("not-a-directory", b"file entry")
            .expect("invalid PATH entry must be created");
        fixture_dir
            .create_dir("bin")
            .expect("binary directory must be created");
        let binary_directory = fixture.path().join("bin");
        whitaker_installer::test_utils::dependency_binary_helpers::write_fake_binary(
            &binary_directory.join("dylint-link"),
            true,
        )
        .expect("fake linker must be executable");

        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let path_entries = std::iter::once(invalid_entry.clone())
            .chain(std::iter::once(binary_directory.clone()))
            .chain(std::env::split_paths(&original_path));
        let path = std::env::join_paths(path_entries).expect("fixture PATH must be valid");

        PathScanFixture {
            _directory: fixture,
            path,
            invalid_entry,
            binary_directory,
        }
    }

    /// Run this test binary as the tracing probe under one verbosity mode.
    #[cfg(unix)]
    fn run_path_scan_child(mode: &str, path: &std::ffi::OsStr) -> std::process::Output {
        std::process::Command::new(
            std::env::current_exe().expect("test executable must be discoverable"),
        )
        .args([
            "--exact",
            "tracing_config::tests::cli_flags_filter_path_scan_diagnostics_on_stderr",
            "--nocapture",
        ])
        .env(PATH_SCAN_PROBE_ENV, mode)
        .env("PATH", path)
        .output()
        .expect("isolated CLI tracing probe must run")
    }

    /// Verify the probe exited successfully.
    #[cfg(unix)]
    fn assert_probe_status(mode: &str, output: &std::process::Output) {
        assert!(
            output.status.success(),
            "{mode} tracing probe must succeed: {output:?}"
        );
    }

    /// Verify only the verbose mode reports the PATH-scan diagnostic.
    #[cfg(unix)]
    fn assert_probe_diagnostics(mode: &str, output: &std::process::Output) {
        let stderr = std::str::from_utf8(&output.stderr).expect("stderr must be UTF-8");
        let is_verbose = mode == "verbose";
        assert_eq!(stderr.contains("skipping PATH scan candidate"), is_verbose);
        assert_eq!(stderr.contains("binary_name=\"dylint-link\""), is_verbose);
        assert_eq!(
            stderr.contains("failure_category=\"directory_open\""),
            is_verbose
        );
    }

    /// Verify the diagnostic redacts fixture paths and stays plain text.
    #[cfg(unix)]
    fn assert_probe_redaction(output: &std::process::Output, fixture: &PathScanFixture) {
        let stderr = std::str::from_utf8(&output.stderr).expect("stderr must be UTF-8");
        let stdout = std::str::from_utf8(&output.stdout).expect("stdout must be UTF-8");
        assert!(!stderr.contains(fixture.invalid_entry.to_string_lossy().as_ref()));
        assert!(!stderr.contains(fixture.binary_directory.to_string_lossy().as_ref()));
        assert!(!stdout.contains("skipping PATH scan candidate"));
        assert!(!stderr.contains('\u{1b}'), "stderr must omit ANSI escapes");
    }

    /// Run the child-only probe that exercises the production PATH scan event.
    #[cfg(unix)]
    fn run_path_scan_probe() {
        let mode = std::env::var(PATH_SCAN_PROBE_ENV);
        let flags: &[&str] = match mode.as_deref() {
            Ok("verbose") => &["-v"],
            Ok("quiet") => &["-q"],
            _ => &[],
        };
        let arguments = std::iter::once("whitaker-installer").chain(flags.iter().copied());
        let cli = Cli::try_parse_from(arguments).expect("probe arguments must match the CLI");
        configure_tracing(cli.install_args())
            .expect("child probe tracing subscriber must initialize");
        drop(crate::install_flow::check_dylint_tools_for_command(
            &whitaker_installer::deps::SystemCommandExecutor,
        ));
    }

    /// Verify CLI initialization sends diagnostics to stderr in its own process.
    #[test]
    fn tracing_writes_to_stderr() {
        const PROBE_ENV: &str = "WHITAKER_TRACING_STDERR_PROBE";
        if std::env::var_os(PROBE_ENV).is_some() {
            emit_stderr_probe();
            return;
        }
        let output = std::process::Command::new(
            std::env::current_exe().expect("test executable must be discoverable"),
        )
        .args([
            "--exact",
            "tracing_config::tests::tracing_writes_to_stderr",
            "--nocapture",
        ])
        .env(PROBE_ENV, "1")
        .output()
        .expect("isolated tracing probe must run");
        assert_stderr_diagnostic(&output);
    }

    /// Emit the diagnostic the parent process looks for.
    fn emit_stderr_probe() {
        configure_tracing(&InstallArgs::default())
            .expect("child probe tracing subscriber must initialize");
        tracing::warn!("installer stderr diagnostic");
    }

    /// Verify the diagnostic reached stderr, and only stderr, as plain text.
    fn assert_stderr_diagnostic(output: &std::process::Output) {
        let stderr = std::str::from_utf8(&output.stderr).expect("stderr must be UTF-8");
        let stdout = std::str::from_utf8(&output.stdout).expect("stdout must be UTF-8");
        assert!(
            output.status.success(),
            "tracing probe must succeed: {output:?}"
        );
        assert!(stderr.contains("installer stderr diagnostic"));
        assert!(!stdout.contains("installer stderr diagnostic"));
        assert!(!stderr.contains('\u{1b}'), "stderr must omit ANSI escapes");
    }

    /// Verify a pre-installed global subscriber becomes a reported CLI error.
    #[test]
    fn tracing_initialization_failure_is_reported() {
        const PROBE_ENV: &str = "WHITAKER_TRACING_INITIALIZATION_FAILURE_PROBE";
        if std::env::var_os(PROBE_ENV).is_some() {
            tracing::subscriber::set_global_default(tracing_subscriber::registry())
                .expect("probe subscriber must be installed first");
            let error = configure_tracing(&InstallArgs::default())
                .expect_err("a second global subscriber must be rejected");
            assert!(
                error
                    .to_string()
                    .contains("failed to initialize diagnostic tracing")
            );
            return;
        }

        let output = std::process::Command::new(
            std::env::current_exe().expect("test executable must be discoverable"),
        )
        .args([
            "--exact",
            "tracing_config::tests::tracing_initialization_failure_is_reported",
            "--nocapture",
        ])
        .env(PROBE_ENV, "1")
        .output()
        .expect("isolated tracing failure probe must run");
        assert!(
            output.status.success(),
            "tracing failure probe must report the error: {output:?}"
        );
    }

    /// Emit one event at every level so the active filter can be observed.
    ///
    /// Each level lives in its own function because a tracing macro expands to
    /// branches that Clippy counts towards the caller's cognitive complexity.
    fn emit_all_levels() {
        emit_error_event();
        emit_warning_event();
        emit_info_event();
        emit_debug_event();
        emit_trace_event();
    }

    /// Emit the error-level diagnostic sample.
    fn emit_error_event() {
        tracing::error!("error diagnostic");
    }

    /// Emit the warning-level diagnostic sample.
    fn emit_warning_event() {
        tracing::warn!("warning diagnostic");
    }

    /// Emit the info-level diagnostic sample.
    fn emit_info_event() {
        tracing::info!("info diagnostic");
    }

    /// Emit the debug-level diagnostic sample.
    fn emit_debug_event() {
        tracing::debug!("debug diagnostic");
    }

    /// Emit the trace-level diagnostic sample.
    fn emit_trace_event() {
        tracing::trace!("trace diagnostic");
    }

    /// Verify each verbosity policy through emitted, formatted events.
    #[rstest]
    #[case::quiet(true, 0, &["error diagnostic"])]
    #[case::default(false, 0, &["error diagnostic", "warning diagnostic"])]
    #[case::verbose(false, 1, &["error diagnostic", "warning diagnostic", "info diagnostic", "debug diagnostic"])]
    #[case::repeated_verbose(false, 2, &["error diagnostic", "warning diagnostic", "info diagnostic", "debug diagnostic"])]
    #[case::quiet_overrides_verbose(true, 2, &["error diagnostic"])]
    fn tracing_respects_verbosity_without_ansi(
        #[case] quiet: bool,
        #[case] verbosity: u8,
        #[case] expected_messages: &[&str],
    ) {
        let args = InstallArgs {
            quiet,
            verbosity,
            ..InstallArgs::default()
        };
        let mut capture = tempfile::tempfile().expect("diagnostic capture file must be created");
        let subscriber = tracing_formatter(&args)
            .with_writer(capture.try_clone().expect("capture file must be cloned"))
            .finish();
        tracing::subscriber::with_default(subscriber, emit_all_levels);
        capture.rewind().expect("capture file must rewind");
        let mut output = String::new();
        capture
            .read_to_string(&mut output)
            .expect("formatted diagnostics must be UTF-8");
        let messages = output
            .lines()
            .map(|line| {
                line.split_once(": ")
                    .expect("event must include its formatted message")
                    .1
            })
            .collect::<Vec<_>>();
        assert_eq!(messages, expected_messages);
        assert!(
            !output.contains('\u{1b}'),
            "diagnostics must omit ANSI escapes"
        );
    }
}
