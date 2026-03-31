//! Package dependency binaries and shared provenance assets for release uploads.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use thiserror::Error;
use whitaker_installer::dependency_binaries::{
    find_dependency_binary, required_dependency_binaries,
};
use whitaker_installer::dependency_packaging::{
    DependencyPackageParams, DependencyPackagingError, package_dependency_binary,
    write_provenance_markdown,
};
use whitaker_installer::installer_packaging::TargetTriple;

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
}

/// Parse command-line arguments, execute the requested subcommand, and report
/// failures to stderr.
fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

/// Execute one dependency-binary packaging command.
fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Package {
            package,
            target,
            binary_path,
            output_dir,
        } => {
            let dependency = find_dependency_binary(&package)
                .map_err(|error| CliError::Manifest(error.to_string()))?
                .cloned()
                .ok_or(CliError::UnknownPackage(package))?;
            let target = TargetTriple::try_from(target.as_str())?;
            let output = package_dependency_binary(DependencyPackageParams {
                dependency,
                target,
                binary_path,
                output_dir,
            })?;
            println!("Created {}", output.archive_path.display());
        }
        Command::Provenance { output_dir } => {
            let dependencies = required_dependency_binaries()
                .map_err(|error| CliError::Manifest(error.to_string()))?;
            let output = write_provenance_markdown(&output_dir, dependencies)?;
            println!("Created {}", output.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use whitaker_installer::dependency_binaries::provenance_filename;

    #[test]
    fn run_package_command_rejects_invalid_target() {
        let temp_dir = tempdir().expect("temp dir");
        let binary_path = temp_dir.path().join("cargo-dylint");
        std::fs::write(&binary_path, b"fake binary").expect("write binary");

        let cli = Cli {
            command: Command::Package {
                package: "cargo-dylint".to_owned(),
                target: "invalid-target-triple".to_owned(),
                binary_path,
                output_dir: temp_dir.path().join("dist"),
            },
        };

        let result = run(cli);
        assert!(matches!(result, Err(CliError::Target(_))));
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
    fn run_package_command_rejects_unknown_package() {
        let temp_dir = tempdir().expect("temp dir");
        let binary_path = temp_dir.path().join("fake-binary");
        std::fs::write(&binary_path, b"fake binary").expect("write binary");

        let cli = Cli {
            command: Command::Package {
                package: "missing-package".to_owned(),
                target: "x86_64-unknown-linux-gnu".to_owned(),
                binary_path,
                output_dir: temp_dir.path().join("dist"),
            },
        };

        let result = run(cli);
        assert!(
            matches!(result, Err(CliError::UnknownPackage(package)) if package == "missing-package")
        );
    }
}
