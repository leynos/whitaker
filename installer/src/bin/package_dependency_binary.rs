//! Package dependency binaries and shared provenance assets for release uploads.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use thiserror::Error;
use whitaker_installer::{
    dependency_binaries::{find_dependency_binary, required_dependency_binaries},
    dependency_packaging::{
        DependencyPackageParams,
        DependencyPackagingError,
        package_dependency_binary,
        write_provenance_markdown,
    },
    installer_packaging::TargetTriple,
    output::write_stderr_line,
};

/// Package repository-hosted dependency binaries for release publication.
#[derive(Parser, Debug)]
#[command(name = "whitaker-package-dependency-binary", version)]
#[command(about = "Package dependency binaries and provenance assets")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Supported release-packaging subcommands.
#[derive(Subcommand, Debug)]
enum Command {
    /// Package one dependency executable for one target.
    Package {
        /// Package name from `dependency-binaries.toml`.
        #[arg(long)]
        package: String,

        /// Target triple for the built executable.
        #[arg(long)]
        target: String,

        /// Path to the compiled executable.
        #[arg(long)]
        binary_path: PathBuf,

        /// Directory where the archive should be written.
        #[arg(long)]
        output_dir: PathBuf,
    },

    /// Generate the shared provenance/licence sidecar.
    Provenance {
        /// Directory where the Markdown document should be written.
        #[arg(long)]
        output_dir: PathBuf,
    },
}

/// Inputs required to package one dependency executable.
struct PackageRequest {
    package: String,
    target: String,
    binary_path: PathBuf,
    output_dir: PathBuf,
}

/// Command-line errors returned by the dependency-binary packaging tool.
#[derive(Debug, Error)]
enum CliError {
    #[error("unknown dependency package: {0}")]
    UnknownPackage(String),

    #[error("dependency manifest error: {0}")]
    Manifest(String),

    #[error("{0}")]
    Packaging(#[from] DependencyPackagingError),

    #[error("{0}")]
    Target(#[from] whitaker_installer::artefact::error::ArtefactError),

    #[error("failed to write to stdout: {0}")]
    Stdout(#[from] std::io::Error),
}

/// Parse command-line arguments, execute the requested subcommand, and report
/// failures to stderr.
fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        write_stderr_line(&mut std::io::stderr(), format!("error: {error}"));
        std::process::exit(1);
    }
}

/// Execute one dependency-binary packaging command.
fn run(cli: Cli) -> Result<(), CliError> { run_with_stdout(cli, &mut std::io::stdout()) }

/// Execute a subcommand and write its success report to `stdout`.
///
/// # Errors
///
/// Return the validation, packaging, or output error from the command.
fn run_with_stdout<W: Write>(cli: Cli, stdout: &mut W) -> Result<(), CliError> {
    match cli.command {
        Command::Package {
            package,
            target,
            binary_path,
            output_dir,
        } => run_package(
            PackageRequest {
                package,
                target,
                binary_path,
                output_dir,
            },
            stdout,
        ),
        Command::Provenance { output_dir } => run_provenance(&output_dir, stdout),
    }
}

/// Package a single dependency binary into a release archive.
fn run_package(request: PackageRequest, stdout: &mut impl Write) -> Result<(), CliError> {
    let dependency = find_dependency_binary(&request.package)
        .map_err(|error| CliError::Manifest(error.to_string()))?
        .cloned()
        .ok_or(CliError::UnknownPackage(request.package))?;
    let target_triple = TargetTriple::try_from(request.target.as_str())?;
    let output = package_dependency_binary(&DependencyPackageParams {
        dependency,
        target: target_triple,
        binary_path: request.binary_path,
        output_dir: request.output_dir,
    })?;
    writeln!(stdout, "Created {}", output.archive_path.display())?;
    Ok(())
}

/// Write the dependency provenance summary into `output_dir`.
fn run_provenance(output_dir: &Path, stdout: &mut impl Write) -> Result<(), CliError> {
    let dependencies =
        required_dependency_binaries().map_err(|error| CliError::Manifest(error.to_string()))?;
    let output = write_provenance_markdown(output_dir, dependencies)?;
    writeln!(stdout, "Created {}", output.display())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the dependency-binary packaging command.

    use rstest::rstest;
    use tempfile::tempdir;
    use whitaker_installer::dependency_binaries::provenance_filename;

    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum ExpectedPackageError {
        Target,
        UnknownPackage(&'static str),
    }

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }

    #[rstest]
    #[case::invalid_target("cargo-dylint", "invalid-target-triple", ExpectedPackageError::Target)]
    #[case::unknown_package(
        "missing-package",
        "x86_64-unknown-linux-gnu",
        ExpectedPackageError::UnknownPackage("missing-package")
    )]
    fn run_package_command_rejects_invalid_input(
        #[case] package: &str,
        #[case] target: &str,
        #[case] expected_error: ExpectedPackageError,
    ) {
        let temp_dir = tempdir().expect("temp dir");
        let binary_path = temp_dir.path().join("fake-binary");
        std::fs::write(&binary_path, b"fake binary").expect("write binary");

        let cli = Cli {
            command: Command::Package {
                package: package.to_owned(),
                target: target.to_owned(),
                binary_path,
                output_dir: temp_dir.path().join("dist"),
            },
        };

        let error = run(cli).expect_err("invalid package input should be rejected");
        match (expected_error, error) {
            (ExpectedPackageError::Target, CliError::Target(_)) => {}
            (ExpectedPackageError::UnknownPackage(expected), CliError::UnknownPackage(actual)) => {
                assert_eq!(actual, expected)
            }
            (expected, actual) => {
                panic!("expected {expected:?}, got {actual:?}");
            }
        }
    }

    #[test]
    fn run_package_command_creates_archive() {
        let temp_dir = tempdir().expect("temp dir");
        let binary_path = temp_dir.path().join("cargo-dylint");
        std::fs::write(&binary_path, b"fake binary").expect("write binary");

        // Look up the expected version from the manifest to avoid hardcoding.
        let dependency = find_dependency_binary("cargo-dylint")
            .expect("manifest should be parseable")
            .expect("cargo-dylint should be in manifest");
        let expected_filename = format!(
            "dist/cargo-dylint-x86_64-unknown-linux-gnu-v{}.tgz",
            dependency.version()
        );

        let cli = Cli {
            command: Command::Package {
                package: "cargo-dylint".to_owned(),
                target: "x86_64-unknown-linux-gnu".to_owned(),
                binary_path,
                output_dir: temp_dir.path().join("dist"),
            },
        };

        let result = run(cli);
        assert!(
            result.is_ok(),
            "expected package command to succeed: {result:?}"
        );
        assert!(temp_dir.path().join(expected_filename).is_file());
    }

    #[test]
    fn run_provenance_command_writes_markdown() {
        let temp_dir = tempdir().expect("temp dir");
        let output_dir = temp_dir.path().join("dist");

        let cli = Cli {
            command: Command::Provenance {
                output_dir: output_dir.clone(),
            },
        };

        let result = run(cli);
        assert!(
            result.is_ok(),
            "expected provenance command to succeed: {result:?}"
        );
        assert!(output_dir.join(provenance_filename()).is_file());
    }

    #[test]
    fn package_command_reports_broken_stdout() {
        let temp_dir = tempdir().expect("temp dir");
        let binary_path = temp_dir.path().join("cargo-dylint");
        std::fs::write(&binary_path, b"fake binary").expect("write binary");
        let cli = Cli {
            command: Command::Package {
                package: "cargo-dylint".to_owned(),
                target: "x86_64-unknown-linux-gnu".to_owned(),
                binary_path,
                output_dir: temp_dir.path().join("dist"),
            },
        };

        let error = run_with_stdout(cli, &mut BrokenPipeWriter)
            .expect_err("broken stdout should fail the package command");

        assert!(matches!(
            error,
            CliError::Stdout(source) if source.kind() == std::io::ErrorKind::BrokenPipe
        ));
    }

    #[test]
    fn provenance_command_reports_broken_stdout() {
        let temp_dir = tempdir().expect("temp dir");
        let cli = Cli {
            command: Command::Provenance {
                output_dir: temp_dir.path().join("dist"),
            },
        };

        let error = run_with_stdout(cli, &mut BrokenPipeWriter)
            .expect_err("broken stdout should fail the provenance command");

        assert!(matches!(
            error,
            CliError::Stdout(source) if source.kind() == std::io::ErrorKind::BrokenPipe
        ));
    }
}
