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

/// Enable bounded stderr diagnostics according to install verbosity flags.
pub(crate) fn configure_tracing(args: &InstallArgs) {
    let _ = tracing_formatter(args).try_init();
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

    use clap::Parser;
    use rstest::rstest;
    use whitaker_installer::cli::Cli;

    use super::*;

    const PATH_SCAN_PROBE_ENV: &str = "WHITAKER_TRACING_PATH_SCAN_PROBE";

    /// Verify CLI flags filter the real PATH-scan diagnostic in a child process.
    #[cfg(unix)]
    #[test]
    fn cli_flags_filter_path_scan_diagnostics_on_stderr() {
        if std::env::var_os(PATH_SCAN_PROBE_ENV).is_some() {
            run_path_scan_probe();
            return;
        }

        use camino::Utf8Path;
        use cap_std::{ambient_authority, fs_utf8::Dir};

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
        let fake_linker = binary_directory.join("dylint-link");
        whitaker_installer::test_utils::dependency_binary_helpers::write_fake_binary(
            &fake_linker,
            true,
        )
        .expect("fake linker must be executable");

        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let original_paths = std::env::split_paths(&original_path);
        let path_entries = std::iter::once(invalid_entry.clone())
            .chain(std::iter::once(binary_directory.clone()))
            .chain(original_paths);
        let path = std::env::join_paths(path_entries).expect("fixture PATH must be valid");

        for mode in ["default", "verbose", "quiet"] {
            let output = std::process::Command::new(
                std::env::current_exe().expect("test executable must be discoverable"),
            )
            .args([
                "--exact",
                "tracing_config::tests::cli_flags_filter_path_scan_diagnostics_on_stderr",
                "--nocapture",
            ])
            .env(PATH_SCAN_PROBE_ENV, mode)
            .env("PATH", &path)
            .output()
            .expect("isolated CLI tracing probe must run");
            assert!(
                output.status.success(),
                "{mode} tracing probe must succeed: {output:?}"
            );
            let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");
            let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
            let has_path_diagnostic = stderr.contains("skipping PATH scan candidate");
            let is_verbose = mode == "verbose";
            assert_eq!(has_path_diagnostic, is_verbose);
            assert_eq!(stderr.contains("binary_name=\"dylint-link\""), is_verbose);
            assert_eq!(
                stderr.contains("failure_category=\"directory_open\""),
                is_verbose
            );
            assert!(!stderr.contains(invalid_entry.to_string_lossy().as_ref()));
            assert!(!stderr.contains(binary_directory.to_string_lossy().as_ref()));
            assert!(!stdout.contains("skipping PATH scan candidate"));
            assert!(!stderr.contains('\u{1b}'), "stderr must omit ANSI escapes");
        }
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
        configure_tracing(cli.install_args());
        let _ = whitaker_installer::deps::check_dylint_tools(
            &whitaker_installer::deps::SystemCommandExecutor,
        );
    }

    /// Verify CLI initialization sends diagnostics to stderr in its own process.
    #[test]
    fn tracing_writes_to_stderr() {
        const PROBE_ENV: &str = "WHITAKER_TRACING_STDERR_PROBE";
        if std::env::var_os(PROBE_ENV).is_some() {
            configure_tracing(&InstallArgs::default());
            tracing::warn!("installer stderr diagnostic");
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
        assert!(
            output.status.success(),
            "tracing probe must succeed: {output:?}"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr must be UTF-8");
        let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
        assert!(stderr.contains("installer stderr diagnostic"));
        assert!(!stdout.contains("installer stderr diagnostic"));
        assert!(!stderr.contains('\u{1b}'), "stderr must omit ANSI escapes");
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
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!("error diagnostic");
            tracing::warn!("warning diagnostic");
            tracing::info!("info diagnostic");
            tracing::debug!("debug diagnostic");
            tracing::trace!("trace diagnostic");
        });
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
