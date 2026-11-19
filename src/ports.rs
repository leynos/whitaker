//! Application ports separating CLI policy from installer and process adapters.

use std::{ffi::OsString, io, io::Write};

use thiserror::Error;
use whitaker_installer::{
    bootstrap::{BootstrapError, PreparedRun},
    cli::{InstallArgs, ListArgs},
    error::InstallerError,
    installation_record::InstallationRecordError,
    scanner::InstalledLints,
};

use crate::diagnostics::DiagnosticSettings;

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
    /// A library-path candidate exists but could not be used.
    ///
    /// A candidate that is merely absent is dropped silently, because an
    /// earlier install may have been removed. A candidate that exists but
    /// cannot be inspected or represented is a real failure, and forwarding
    /// with no library path would hide it.
    #[error("could not use the Dylint library path candidates: {message}")]
    LibraryPathUnusable {
        /// Human-readable details of each unusable candidate.
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

/// A validated installation a `check` run may lint with.
///
/// The value is produced only by a successful readiness check, so holding one
/// is the evidence that the installation was inspected before Dylint ran.
#[derive(Debug, Clone)]
pub enum CheckPrepared {
    /// The installed bootstrap is ready, and knows where its libraries are.
    Bootstrap(PreparedRun),
    /// The user's own library path holds the suite, so Dylint runs directly.
    Direct(DirectLibraryPath),
}

pub use crate::library_path::DirectLibraryPath;

/// Why a readiness check did not approve an installation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CheckPreflightError {
    /// The installed bootstrap reported what is wrong.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapError),
    /// The user managed the library path, and it names no Whitaker suite.
    ///
    /// Only the user can repair this: installing Whitaker elsewhere cannot
    /// change a directory the user chose.
    #[error("the Dylint library path names no Whitaker suite library")]
    UserPathHasNoSuite,
    /// The user managed the library path, and it names more than one suite.
    ///
    /// Dylint refuses to choose between them and exits with the same status a
    /// lint denial uses, so the ambiguity is refused before Dylint starts.
    #[error("the Dylint library path names more than one Whitaker suite library")]
    UserPathAmbiguous,
}

/// Why a repair attempt did not complete.
///
/// The variants separate what a person can act on: an installer that ran and
/// failed, a lock that another process held, and a host that could not offer
/// the lock at all. Folding them together would turn "wait for the other
/// install" into "reinstall Whitaker".
#[derive(Debug, Error)]
pub enum CheckRepairError {
    /// The installer ran and did not complete.
    #[error("installing Whitaker did not complete: {message}")]
    Install {
        /// Human-readable installer details.
        message: String,
    },
    /// The installation lock could not be taken, so nothing was installed.
    #[error("could not take the Whitaker installation lock: {message}")]
    Lock {
        /// Human-readable lock error details.
        message: String,
    },
    /// The platform data directory could not be determined.
    #[error("the platform data directory could not be determined")]
    DataDirectory,
    /// The host failed before provisioning could start.
    #[error("the host failed before provisioning: {message}")]
    Host {
        /// Human-readable host error details.
        message: String,
    },
}

/// Why a lint run could not report a status.
#[derive(Debug, Error)]
pub enum CheckRunError {
    /// The installation or the child process failed.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapError),
    /// `cargo dylint` could not be started on the direct path.
    #[error("could not start cargo dylint: {source}")]
    Spawn {
        /// The operating-system process error.
        #[source]
        source: io::Error,
    },
    /// Whitaker could not write its own output.
    #[error("could not write command output")]
    Output {
        /// The operating-system write error.
        #[source]
        source: io::Error,
    },
}

/// Inspects, and runs Dylint against, the Whitaker installation.
///
/// This is the `check` port rather than a reuse of [`LintRunner`] because the
/// two commands need opposite guarantees. Forwarding runs whatever Dylint
/// finds and reports its status; `check` refuses to start Dylint until an
/// inspection has approved the installation, and returns the prepared value
/// that proves it.
pub trait CheckServices {
    /// Inspect the installation and return the evidence that Dylint may run.
    ///
    /// # Errors
    ///
    /// Returns an error naming what is wrong with the installation, or with
    /// the user's own library path when that path is authoritative.
    fn preflight(&self) -> Result<CheckPrepared, CheckPreflightError>;

    /// Report whether the environment handed Dylint's libraries to the user.
    ///
    /// The command layer derives its repair policy from this answer, so the
    /// query is a fact about the environment rather than a decision: only the
    /// adapter reads the variable itself.
    fn library_path_is_user_managed(&self) -> bool;

    /// Provision the installation, then return once it should be ready.
    ///
    /// The caller holds no lock and passes no policy: the adapter re-checks the
    /// installation under the installation lock, so two concurrent checks
    /// install once rather than twice. `settings` is the root command's own
    /// diagnostic policy, so an implicit install is as quiet or as verbose as
    /// the invocation that asked for it.
    ///
    /// # Errors
    ///
    /// Returns [`CheckRepairError`] when provisioning failed, when the lock
    /// could not be taken, or when the host failed before it could start.
    fn repair(
        &self,
        settings: DiagnosticSettings,
        stderr: &mut dyn Write,
    ) -> Result<(), CheckRepairError>;

    /// Run Dylint against a prepared installation and report its status.
    ///
    /// # Errors
    ///
    /// Returns an error when the installation stopped being usable, when the
    /// child cannot be started, or when Whitaker cannot write its own output.
    fn check_run(
        &self,
        prepared: CheckPrepared,
        args: &[OsString],
        stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError>;
}

/// Groups the independently replaceable command adapters.
pub struct Services<'a> {
    /// Installation adapter.
    pub install: &'a dyn InstallService,
    /// Installed-lint inventory adapter.
    pub inventory: &'a dyn LintInventory,
    /// Dylint subprocess adapter.
    pub lint_runner: &'a dyn LintRunner,
    /// `check` readiness and run adapter.
    pub check: &'a dyn CheckServices,
}
