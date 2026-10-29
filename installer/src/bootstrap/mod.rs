//! The installed bootstrap that `whitaker` and `whitaker-ls` run through.
//!
//! An installation lays its Dylint inputs out beside the managed Whitaker
//! clone, never inside it, because the clone directory is itself a Git
//! working tree:
//!
//! ```text
//! <data root>/whitaker-bootstrap/
//!   bootstrap.json                        what was installed
//!   clippy/<commit>/                      verified Clippy snapshot
//!   drivers/dylint-<version>/clippy-<commit>/
//!     .<toolchain>.lock, .<toolchain>.ready
//!     <toolchain>/dylint-driver           built by Dylint on first use
//! ```
//!
//! Every path is derived from the current platform data directory when a
//! command runs, so a relocated data root keeps working. [`run`] is the one
//! dispatcher the wrapper scripts call: it validates the installation, exports
//! the Dylint variables, serializes cold driver construction, and returns the
//! child's exit status unchanged.

mod child;
mod driver;
mod error;
mod metadata;
mod probe;
mod run;
mod telemetry;

pub use child::exit_code;
pub use driver::{DriverIdentity, DriverSlot};
pub use error::BootstrapError;
pub use metadata::{BOOTSTRAP_SCHEMA, BootstrapMetadata, StoredPath};
pub use probe::is_present_dir;
pub use run::{RunOutcome, run, run_with_cargo};

/// What `whitaker-installer run` prints once a run has built the driver.
///
/// The release verification scripts count this line to prove a cold run built
/// the driver once and a warm run not at all, so keep its wording stable.
pub const BUILT_DRIVER_NOTICE: &str =
    "whitaker: built the Dylint driver on first use; later runs reuse it";

use crate::dirs::BaseDirs;
use camino::{Utf8Path, Utf8PathBuf};

/// Where one installation keeps its bootstrap, resolved from the data root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapLayout {
    data_root: Utf8PathBuf,
    root: Utf8PathBuf,
}

impl BootstrapLayout {
    /// Resolve the layout from the platform directories.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::NoDataDirectory`] when the platform data
    /// directory is unknown or not valid UTF-8.
    pub fn from_dirs(dirs: &dyn BaseDirs) -> Result<Self, BootstrapError> {
        let root = dirs
            .bootstrap_dir()
            .and_then(|path| Utf8PathBuf::try_from(path).ok())
            .ok_or(BootstrapError::NoDataDirectory)?;
        Ok(Self::at(&root))
    }

    /// Describe a layout rooted at `root`, whose parent is the data root.
    ///
    /// # Examples
    ///
    /// ```
    /// use camino::Utf8Path;
    /// use whitaker_installer::bootstrap::BootstrapLayout;
    ///
    /// let layout = BootstrapLayout::at(Utf8Path::new("/data/whitaker-bootstrap"));
    /// assert_eq!(layout.data_root(), Utf8Path::new("/data"));
    /// assert_eq!(
    ///     layout.driver_dir("6.3.0", "e4608a25"),
    ///     Utf8Path::new("/data/whitaker-bootstrap/drivers/dylint-6.3.0/clippy-e4608a25"),
    /// );
    /// ```
    #[must_use]
    pub fn at(root: &Utf8Path) -> Self {
        let data_root = root.parent().unwrap_or(root).to_owned();
        Self {
            data_root,
            root: root.to_owned(),
        }
    }

    /// The data root that relative metadata paths resolve against.
    #[must_use]
    pub fn data_root(&self) -> &Utf8Path {
        &self.data_root
    }

    /// The bootstrap directory itself.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// The directory holding snapshot generations.
    #[must_use]
    pub fn clippy_root(&self) -> Utf8PathBuf {
        self.root.join("clippy")
    }

    /// The snapshot generation for `commit`.
    #[must_use]
    pub fn snapshot(&self, commit: &str) -> Utf8PathBuf {
        self.clippy_root().join(commit)
    }

    /// The driver slot for one Dylint release and snapshot.
    #[must_use]
    pub fn driver_dir(&self, dylint_version: &str, commit: &str) -> Utf8PathBuf {
        self.root
            .join("drivers")
            .join(format!("dylint-{dylint_version}"))
            .join(format!("clippy-{commit}"))
    }

    /// The installation record.
    #[must_use]
    pub fn metadata_path(&self) -> Utf8PathBuf {
        self.root.join("bootstrap.json")
    }
}

#[cfg(test)]
mod tests;
