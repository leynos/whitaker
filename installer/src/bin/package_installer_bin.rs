//! Packaging binary for installer release distribution.
//!
//! Thin CLI wrapper around
//! [`whitaker_installer::installer_packaging::package_installer`] invoked
//! by the release CI workflow to create binstall-compatible archives.

use clap::Parser;
use std::path::PathBuf;
use thiserror::Error;
use whitaker_installer::artefact::error::ArtefactError;
use whitaker_installer::installer_packaging::{
    InstallerPackagingError, TargetTriple, Version, package_installer,
};
use whitaker_installer::output::write_stderr_line;

/// Package the `whitaker-installer` binary into a release archive.
///
/// Creates a `.tgz` (or `.zip` for Windows) archive following the
/// binstall naming convention, suitable for upload to a GitHub Release.
#[derive(Parser, Debug)]
#[command(name = "whitaker-package-installer", version)]
#[command(about = "Package the whitaker-installer binary for release")]
struct Cli {
    /// Crate version (e.g. "0.2.1").
    #[arg(long = "crate-version")]
    crate_version: String,

    /// Target triple (e.g. "x86_64-unknown-linux-gnu").
    #[arg(long)]
    target: String,

    /// Path to the compiled installer binary.
    #[arg(long)]
    binary_path: PathBuf,

    /// Output directory for the archive.
    #[arg(long)]
    output_dir: PathBuf,
}

/// Errors returned by the packaging CLI.
#[derive(Debug, Error)]
enum CliError {
    /// An error during archive creation.
    #[error("{0}")]
    Packaging(#[from] InstallerPackagingError),

    /// An invalid target triple was provided.
    #[error("{0}")]
    Artefact(#[from] ArtefactError),

    /// Failed to write the success report to standard output.
    #[error("failed to write to stdout: {0}")]
    Stdout(#[from] std::io::Error),
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        write_stderr_line(&mut std::io::stderr(), format!("error: {err}"));
        std::process::exit(1);
    }
}

/// Validate inputs and delegate to the packaging library.
fn run(cli: Cli) -> Result<(), CliError> {
    run_with_stdout(cli, &mut std::io::stdout())
}

/// Package the installer and write the success report to `stdout`.
///
/// # Errors
///
/// Return the validation, packaging, or output error from the command.
fn run_with_stdout<W: std::io::Write>(cli: Cli, stdout: &mut W) -> Result<(), CliError> {
    let params = whitaker_installer::installer_packaging::InstallerPackageParams {
        version: Version::new(cli.crate_version),
        target: TargetTriple::try_from(cli.target.as_str())?,
        binary_path: cli.binary_path,
        output_dir: cli.output_dir,
    };

    let output = package_installer(&params)?;
    writeln!(stdout, "Created {}", output.archive_path.display())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the installer packaging command.

    use super::*;
    use clap::Parser;
    use rstest::rstest;
    use std::io::Write;

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    const BASE_ARGS: [&str; 9] = [
        "whitaker-package-installer",
        "--crate-version",
        "0.2.1",
        "--target",
        "x86_64-unknown-linux-gnu",
        "--binary-path",
        "/tmp/whitaker-installer",
        "--output-dir",
        "/tmp/dist",
    ];

    #[test]
    fn cli_parses_all_required_args() {
        let cli = Cli::parse_from(BASE_ARGS);
        assert_eq!(cli.crate_version, "0.2.1");
        assert_eq!(cli.target, "x86_64-unknown-linux-gnu");
        assert_eq!(cli.binary_path, PathBuf::from("/tmp/whitaker-installer"));
        assert_eq!(cli.output_dir, PathBuf::from("/tmp/dist"));
    }

    #[rstest]
    #[case::missing_crate_version("--crate-version")]
    #[case::missing_binary_path("--binary-path")]
    fn cli_rejects_missing_required_option(#[case] missing_option: &str) {
        let mut args = BASE_ARGS.to_vec();
        let option_index = args
            .iter()
            .position(|argument| *argument == missing_option)
            .expect("required option should be present in base arguments");
        args.drain(option_index..=option_index + 1);

        Cli::try_parse_from(args).expect_err("clap should reject a missing required option");
    }

    #[rstest]
    fn run_rejects_missing_binary() {
        let cli = Cli {
            crate_version: "0.2.1".to_owned(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            binary_path: PathBuf::from("/tmp/does-not-exist-xyz"),
            output_dir: std::env::temp_dir(),
        };
        let err = run(cli).expect_err("should fail with missing binary");
        assert!(
            matches!(
                err,
                CliError::Packaging(InstallerPackagingError::BinaryNotFound(_))
            ),
            "expected BinaryNotFound, got {err:?}"
        );
    }

    #[test]
    fn run_reports_broken_stdout_after_packaging() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let binary_path = temp_dir.path().join("whitaker-installer");
        std::fs::write(&binary_path, b"fake installer binary").expect("write binary");
        let cli = Cli {
            crate_version: "0.2.9".to_owned(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            binary_path,
            output_dir: temp_dir.path().join("dist"),
        };

        let error = run_with_stdout(cli, &mut BrokenPipeWriter)
            .expect_err("broken stdout should fail the packaging command");

        assert!(matches!(
            error,
            CliError::Stdout(source) if source.kind() == std::io::ErrorKind::BrokenPipe
        ));
    }
}
