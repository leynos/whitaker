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

    use rstest::rstest;

    use super::*;

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
