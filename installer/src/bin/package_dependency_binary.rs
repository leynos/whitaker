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

#[derive(Debug, Error)]
enum CliError {
    #[error("unknown dependency package: {0}")]
    UnknownPackage(String),

    #[error("{0}")]
    Packaging(#[from] DependencyPackagingError),

    #[error("{0}")]
    Target(#[from] whitaker_installer::artefact::error::ArtefactError),
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Package {
            package,
            target,
            binary_path,
            output_dir,
        } => {
            let dependency = find_dependency_binary(&package)
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
            let output = write_provenance_markdown(&output_dir, required_dependency_binaries())?;
            println!("Created {}", output.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_subcommand_parses_required_arguments() {
        let cli = Cli::parse_from([
            "whitaker-package-dependency-binary",
            "package",
            "--package",
            "cargo-dylint",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--binary-path",
            "/tmp/cargo-dylint",
            "--output-dir",
            "/tmp/dist",
        ]);

        match cli.command {
            Command::Package {
                package, target, ..
            } => {
                assert_eq!(package, "cargo-dylint");
                assert_eq!(target, "x86_64-unknown-linux-gnu");
            }
            Command::Provenance { .. } => panic!("expected package command"),
        }
    }

    #[test]
    fn provenance_subcommand_parses_output_dir() {
        let cli = Cli::parse_from([
            "whitaker-package-dependency-binary",
            "provenance",
            "--output-dir",
            "/tmp/dist",
        ]);

        assert!(matches!(cli.command, Command::Provenance { .. }));
    }
}
