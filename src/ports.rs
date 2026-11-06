//! Application ports separating CLI policy from installer and process adapters.

use std::{ffi::OsString, io, io::Write};

use thiserror::Error;
use whitaker_installer::{
    cli::{InstallArgs, ListArgs},
    error::InstallerError,
    installation_record::InstallationRecordError,
    scanner::InstalledLints,
};

/// A semantic error returned by a command adapter.
#[derive(Debug, Error)]
pub enum CliError {
    /// The installer could not complete the requested installation.
    #[error("installation failed: {message}")]
    Install {
        /// Human-readable installer error details.
        message: String,
    },
    /// Installed lint inventory could not be read.
    #[error("could not list installed lints: {message}")]
    Inventory {
        /// Human-readable inventory error details.
        message: String,
    },
    /// The forwarded Dylint process could not be started.
    #[error("could not start cargo dylint: {source}")]
    Forward {
        /// The operating-system process error.
        #[source]
        source: io::Error,
    },
    /// Writing command output failed.
    #[error("could not write command output")]
    Output {
        /// The operating-system write error.
        #[source]
        source: io::Error,
    },
    /// The installation record exists but could not be read.
    ///
    /// A missing record is not an error: it means nothing has been installed
    /// yet. A record that exists but cannot be locked, read, or parsed is a
    /// real failure, and forwarding with an empty library path would hide it.
    #[error("could not read the installation record: {source}")]
    InstallationRecord {
        /// The record-layer failure.
        #[source]
        source: InstallationRecordError,
    },
    /// The active toolchain could not be determined.
    ///
    /// A directory that declares no toolchain is not an error: it means the
    /// caller is outside a checkout. A `rust-toolchain.toml` that exists but
    /// cannot be read or parsed is a real failure, and treating it as "no
    /// toolchain" would silently drop the build and prebuilt candidates for a
    /// toolchain the user explicitly selected.
    #[error("could not detect the active toolchain: {source}")]
    ToolchainDetection {
        /// The toolchain-layer failure.
        #[source]
        source: InstallerError,
    },
    /// The prebuilt library directory for the active toolchain could not be
    /// resolved.
    ///
    /// Falling back to "no candidate" would hide a broken data directory and
    /// let Dylint run without the libraries the user installed.
    #[error("could not resolve the prebuilt library directory: {message}")]
    PrebuiltLibraryPath {
        /// Human-readable installer error details.
        message: String,
    },
    /// The resolved candidate directories could not be joined into one
    /// platform path list.
    ///
    /// Every candidate is checked for representability before it is joined, so
    /// this is a defensive variant rather than an expected outcome. It exists
    /// so a representation failure has a typed report instead of silently
    /// becoming "nothing installed".
    #[error("could not build the Dylint library path: {source}")]
    LibraryPath {
        /// The platform path-list joining error.
        #[source]
        source: std::env::JoinPathsError,
    },
}

/// Input needed to install Whitaker dependencies and lint libraries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    /// Installer options parsed by the shared installer library.
    pub arguments: InstallArgs,
}

/// Input needed to inspect staged lint libraries.
#[derive(Debug, Clone)]
pub struct ListRequest {
    /// List options parsed by the shared installer library.
    pub arguments: ListArgs,
}

/// Staged lint metadata and the toolchain active in the current directory.
#[derive(Debug, Default)]
pub struct StagedInventory {
    /// Installed libraries grouped by toolchain.
    pub installed: InstalledLints,
    /// Active toolchain when the current workspace declares one.
    pub active_toolchain: Option<String>,
}

/// Performs an installation on behalf of the command layer.
pub trait InstallService {
    /// Runs an installation, streaming command output to `stdout` and
    /// diagnostics to `stderr`.
    ///
    /// The installer reserves `stdout` for machine-readable command output
    /// such as the `suite-source` marker; progress and errors go to `stderr`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    ///
    /// use whitaker::{
    ///     adapters::SystemServices,
    ///     ports::{InstallRequest, InstallService},
    /// };
    /// use whitaker_installer::cli::InstallArgs;
    ///
    /// let services = SystemServices::new();
    /// let request = InstallRequest {
    ///     arguments: InstallArgs::default(),
    /// };
    /// let mut stderr = io::sink();
    /// services.install(&request, &mut io::sink(), &mut stderr)?;
    /// # Ok::<(), whitaker::ports::CliError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the installation cannot complete.
    fn install(
        &self,
        request: &InstallRequest,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> Result<(), CliError>;
}

/// Reports the lints currently staged on this machine.
pub trait LintInventory {
    /// Lists staged lints in the requested staging directory.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use clap::Parser as _;
    /// use whitaker::{
    ///     adapters::SystemServices,
    ///     ports::{LintInventory, ListRequest},
    /// };
    /// use whitaker_installer::cli::ListArgs;
    ///
    /// let services = SystemServices::new();
    /// let inventory = services.list(&ListRequest {
    ///     arguments: ListArgs::try_parse_from(["whitaker"])?,
    /// })?;
    /// println!("{} toolchains", inventory.installed.by_toolchain.len());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the staging directory cannot be scanned.
    fn list(&self, request: &ListRequest) -> Result<StagedInventory, CliError>;
}

/// Runs Dylint on behalf of a forwarded invocation.
pub trait LintRunner {
    /// Forwards arguments through the installed bootstrap and returns its exit
    /// code, writing the bootstrap's own diagnostics to `stderr`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::{ffi::OsString, io};
    ///
    /// use whitaker::{adapters::SystemServices, ports::LintRunner};
    ///
    /// let services = SystemServices::new();
    /// let mut stderr = io::sink();
    /// let status = services.forward(&[OsString::from("--all")], &mut stderr)?;
    /// println!("cargo dylint exited with {status}");
    /// # Ok::<(), whitaker::ports::CliError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the invocation cannot be dispatched.
    fn forward(&self, args: &[OsString], stderr: &mut dyn Write) -> Result<i32, CliError>;
}

/// Groups the independently replaceable command adapters.
pub struct Services<'a> {
    /// Installation adapter.
    pub install: &'a dyn InstallService,
    /// Installed-lint inventory adapter.
    pub inventory: &'a dyn LintInventory,
    /// Dylint subprocess adapter.
    pub lint_runner: &'a dyn LintRunner,
}
