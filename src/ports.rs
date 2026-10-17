//! Application ports separating CLI policy from installer and process adapters.

use std::{ffi::OsString, io, io::Write};

use thiserror::Error;
use whitaker_installer::{
    cli::{InstallArgs, ListArgs},
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
    /// Runs an installation, streaming progress and diagnostics to `stderr`.
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
    /// services.install(&request, &mut stderr)?;
    /// # Ok::<(), whitaker::ports::CliError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the installation cannot complete.
    fn install(&self, request: &InstallRequest, stderr: &mut dyn Write) -> Result<(), CliError>;
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
    /// Forwards arguments to `cargo dylint` and returns its exit code.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::ffi::OsString;
    ///
    /// use whitaker::{adapters::SystemServices, ports::LintRunner};
    ///
    /// let services = SystemServices::new();
    /// let status = services.forward(&[OsString::from("--all")])?;
    /// println!("cargo dylint exited with {status}");
    /// # Ok::<(), whitaker::ports::CliError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the subprocess cannot be started.
    fn forward(&self, args: &[OsString]) -> Result<u8, CliError>;
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
