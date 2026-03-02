//! Packaging binary for installer release distribution.
//!
//! Thin CLI wrapper around
//! [`whitaker_installer::installer_packaging::package_installer`] invoked
//! by the release CI workflow to create binstall-compatible archives.

use clap::Parser;
use std::path::PathBuf;
use thiserror::Error;
use whitaker_installer::installer_packaging::{
    InstallerPackagingError, TargetTriple, Version, package_installer,
};

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

    /// An I/O error (e.g. creating the output directory).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

/// Validate inputs, create the output directory, and delegate to the
/// packaging library.
fn run(cli: Cli) -> Result<(), CliError> {
    std::fs::create_dir_all(&cli.output_dir)?;

    let params = whitaker_installer::installer_packaging::InstallerPackageParams {
        version: Version::new(cli.crate_version),
        target: TargetTriple::new(cli.target),
        binary_path: cli.binary_path,
        output_dir: cli.output_dir,
    };

    let output = package_installer(params)?;
    println!("Created {}", output.archive_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rstest::rstest;

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

    #[test]
    fn cli_rejects_missing_crate_version() {
        Cli::try_parse_from([
            "whitaker-package-installer",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--binary-path",
            "/tmp/bin",
            "--output-dir",
            "/tmp/dist",
        ])
        .expect_err("expected clap to reject missing --crate-version");
    }

    #[test]
    fn cli_rejects_missing_binary_path() {
        Cli::try_parse_from([
            "whitaker-package-installer",
            "--crate-version",
            "0.2.1",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--output-dir",
            "/tmp/dist",
        ])
        .expect_err("expected clap to reject missing --binary-path");
    }

    #[rstest]
    fn run_rejects_missing_binary() {
        let cli = Cli {
            crate_version: "0.2.1".to_owned(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            binary_path: PathBuf::from("/tmp/does-not-exist-xyz"),
            output_dir: std::env::temp_dir(),
        };
        assert!(run(cli).is_err(), "should fail with missing binary");
    }
}
