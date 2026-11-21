//! The production adapter set the executable is built around.
//!
//! This type is the composition root. It snapshots the process environment
//! once, when it is built, and every service call in one invocation works from
//! that snapshot. Reading per call instead would let a command see two
//! different environments: `check` may ask readiness twice and repair once,
//! and forwarding resolves its library path after the route is chosen.

use std::ffi::OsString;

use crate::ports::Services;

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
    /// );
    /// ```
    #[must_use]
    pub fn ports(&self) -> Services<'_> {
        Services {
            install: self,
            inventory: self,
            lint_runner: self,
            check: self,
        }
    }
}

impl Default for SystemServices {
    /// Build the adapter set exactly as [`SystemServices::new`] does.
    ///
    /// A derived `Default` would leave every snapshot `None`, silently
    /// dropping the Windows `PATH` adjustment and the route's `CI` advice.
    fn default() -> Self { Self::new() }
}
