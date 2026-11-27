//! The production adapter set the executable is built around.
//!
//! This type is the composition root. It snapshots the process environment
//! once, when it is built, and every service call in one invocation works from
//! that snapshot. Reading per call instead would let a command see two
//! different environments: `check` may ask readiness twice and repair once,
//! and forwarding resolves its library path after the route is chosen.

use std::ffi::OsString;

use camino::Utf8PathBuf;
use whitaker_installer::dirs::SystemBaseDirs;

use super::check::ApprovedRuns;
use crate::ports::{
    PackageQueryError,
    Services,
    WorkspaceFacts,
    WorkspaceQuery,
    WorkspaceQueryRequest,
};

/// The system-backed adapters used by the `whitaker` executable.
///
/// Building this type snapshots the environment; see the module documentation
/// for why the snapshot is taken here rather than per service call.
#[derive(Debug)]
pub struct SystemServices {
    /// This invocation's `PATH`, which the bootstrap extends the child's
    /// `PATH` with on Windows.
    ///
    /// A `None` would silently skip that adjustment.
    pub(super) inherited_path: Option<OsString>,
    /// This invocation's `DYLINT_LIBRARY_PATH`, which selects the route.
    pub(super) dylint_library_path: Option<OsString>,
    /// This invocation's `CI`, which adds the repair advice in continuous
    /// integration.
    pub(super) ci: Option<OsString>,
    /// The run the last successful readiness check approved.
    ///
    /// The `check` port's prepared value names only the route that approved an
    /// installation, because the installer's concrete run must not leave the
    /// adapter. It waits here between readiness and the run that consumes it.
    pub(super) approved: ApprovedRuns,
    /// The platform directories this invocation writes to and reads from.
    ///
    /// Resolved once here rather than at each use, so one invocation cannot see
    /// two different data directories if the environment or the platform's
    /// lookup changes underneath it, and so a test can build the adapter set
    /// against directories it chose.
    pub(super) platform_dirs: Option<SystemBaseDirs>,
    /// The program a user-managed library path is run through.
    ///
    /// Production names the `cargo` on `PATH`. A test names a recording stub
    /// so it can prove the approval gate decides before any child starts: an
    /// assertion on the returned error alone would not show whether a
    /// regressed gate spawned Dylint first and read its status afterwards.
    pub(super) cargo: OsString,
    /// The program the workspace-metadata query runs.
    ///
    /// Production names the `cargo` on `PATH`. A test names a recording stub
    /// so the listing can be exercised without a real workspace.
    pub(super) metadata_cargo: Utf8PathBuf,
}

impl SystemServices {
    /// Creates the production adapter set, snapshotting the environment.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::SystemServices;
    ///
    /// let services = SystemServices::new();
    /// let ports = services.ports();
    /// let _ = ports;
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            inherited_path: std::env::var_os("PATH"),
            dylint_library_path: std::env::var_os("DYLINT_LIBRARY_PATH"),
            ci: std::env::var_os("CI"),
            approved: ApprovedRuns::new(),
            platform_dirs: SystemBaseDirs::new(),
            cargo: OsString::from("cargo"),
            metadata_cargo: Utf8PathBuf::from("cargo"),
        }
    }

    /// Borrows the implementation through the command ports.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::SystemServices;
    ///
    /// let services = SystemServices::new();
    /// let ports = services.ports();
    /// let _ = (
    ///     ports.install,
    ///     ports.inventory,
    ///     ports.lint_runner,
    ///     ports.check,
    ///     ports.packages,
    /// );
    /// ```
    #[must_use]
    pub fn ports(&self) -> Services<'_> {
        Services {
            install: self,
            inventory: self,
            lint_runner: self,
            check: self,
            packages: self,
        }
    }
}

impl WorkspaceQuery for SystemServices {
    /// Read the workspace through this invocation's `cargo`.
    ///
    /// The query spawns a process, so it is the adapter's job rather than the
    /// command layer's; a failure is returned typed and the caller decides
    /// whether it matters.
    fn workspace_facts(
        &self,
        request: &WorkspaceQueryRequest<'_>,
    ) -> Result<WorkspaceFacts, PackageQueryError> {
        super::packages::CargoMetadataQuery {
            cargo: self.metadata_cargo.clone(),
        }
        .workspace_facts(request)
    }
}

impl Default for SystemServices {
    /// Build the adapter set exactly as [`SystemServices::new`] does.
    ///
    /// A derived `Default` would leave every snapshot `None`, silently
    /// dropping the Windows `PATH` adjustment and the route's `CI` advice.
    fn default() -> Self { Self::new() }
}
