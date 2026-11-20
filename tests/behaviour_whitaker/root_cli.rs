//! Fixture helpers only the root-CLI behaviour suite needs.
//!
//! Both root-CLI suites compile `support.rs`, but only this one records extra
//! toolchain staging directories or reads the Clippy and driver exports the
//! stub recorded. Keeping those here means the shared fixture declares nothing
//! one of its two suites cannot use, so neither suite needs a dead-code
//! suppression to compile.

use std::path::{Path, PathBuf};

use whitaker_installer::{dirs::BaseDirs, installation_record::record_installation};

use super::support::BootstrapFixture;

impl BootstrapFixture {
    /// Record another toolchain's staging directory, as an install would.
    ///
    /// # Errors
    ///
    /// Returns an error when the installation record cannot be written.
    pub fn record_toolchain(
        &self,
        toolchain: &str,
        staging: &camino::Utf8Path,
    ) -> Result<(), String> {
        record_installation(&self.dirs(), toolchain, staging)
            .map_err(|error| format!("staging directory should be recorded: {error}"))
    }

    /// The executable's own output directory, for tests that create files.
    pub fn root(&self) -> &Path { self.dir.path() }

    /// The last `DYLINT_CLIPPY_REPO` the stub was given.
    pub fn clippy_repo(&self) -> Option<String> { self.last_recorded("CLIPPY=") }

    /// The last `DYLINT_DRIVER_PATH` the stub was given.
    pub fn driver_path(&self) -> Option<String> { self.last_recorded("DRIVER=") }

    /// The data directories the root binary resolves through `XDG_DATA_HOME`.
    fn dirs(&self) -> FixtureDirs {
        FixtureDirs {
            data: self.data_home.join("whitaker"),
        }
    }
}

/// The minimum [`BaseDirs`] implementation for recording a toolchain.
struct FixtureDirs {
    data: PathBuf,
}

impl BaseDirs for FixtureDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data.clone()) }
}
