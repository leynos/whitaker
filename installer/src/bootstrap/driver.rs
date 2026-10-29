//! The identity-keyed directory one Dylint driver is built into.
//!
//! A slot is `<bootstrap>/drivers/dylint-<version>/clippy-<commit>/`, which
//! `run` exports as `DYLINT_DRIVER_PATH`. Dylint adds the toolchain directory
//! itself. Changing the Dylint release or the Clippy snapshot therefore selects
//! a different slot rather than reusing a driver built from other inputs.
//!
//! Construction is serialized with a lock file per toolchain. A run that finds
//! a ready driver takes the lock shared, so warm runs proceed in parallel. A
//! cold run takes it exclusively, re-checks, builds, and writes the ready
//! marker only after the driver exists. A driver without a marker, left by an
//! interrupted build or by a tool outside Whitaker, is treated as cold.

use super::error::BootstrapError;
use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;
use std::fs::File;

/// The driver executable Dylint writes inside its toolchain directory.
const DRIVER_FILE: &str = if cfg!(windows) {
    "dylint-driver.exe"
} else {
    "dylint-driver"
};

/// One toolchain's driver within an identity-keyed slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverSlot {
    dir: Utf8PathBuf,
    toolchain: String,
}

impl DriverSlot {
    /// Describe the driver for `toolchain` inside the slot `dir`.
    #[must_use]
    pub fn new(dir: &Utf8Path, toolchain: &str) -> Self {
        Self {
            dir: dir.to_owned(),
            toolchain: toolchain.to_owned(),
        }
    }

    /// The directory exported as `DYLINT_DRIVER_PATH`.
    #[must_use]
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    /// Return whether a complete driver is recorded as ready.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.marker_path().is_file() && self.has_driver()
    }

    /// Return whether Dylint has written a driver for this toolchain.
    ///
    /// Dylint names the toolchain directory after the library's toolchain,
    /// which carries the host triple (`nightly-2026-05-28-x86_64-...`), so any
    /// directory for this channel counts. Dylint persists the driver with a
    /// rename, so a present, non-empty file is a complete one.
    #[must_use]
    pub fn has_driver(&self) -> bool {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return false;
        };
        let prefix = format!("{}-", self.toolchain);
        entries.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_toolchain_dir = name == self.toolchain || name.starts_with(&prefix);
            is_toolchain_dir
                && std::fs::metadata(entry.path().join(DRIVER_FILE))
                    .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
        })
    }

    /// Create the slot directory, which Dylint requires to exist.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the directory cannot be created.
    pub fn create(&self) -> Result<(), BootstrapError> {
        std::fs::create_dir_all(&self.dir).map_err(|error| BootstrapError::io(&self.dir, &error))
    }

    /// Block until this process may use a ready driver alongside others.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the lock file cannot be opened or locked.
    pub fn lock_shared(&self) -> Result<File, BootstrapError> {
        let file = self.open_lock()?;
        FileExt::lock_shared(&file)
            .map_err(|error| BootstrapError::io(self.lock_path(), &error))?;
        Ok(file)
    }

    /// Block until this process alone may build the driver.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the lock file cannot be opened or locked.
    pub fn lock_exclusive(&self) -> Result<File, BootstrapError> {
        let file = self.open_lock()?;
        FileExt::lock_exclusive(&file)
            .map_err(|error| BootstrapError::io(self.lock_path(), &error))?;
        Ok(file)
    }

    /// Record that a complete driver exists. Call only under the exclusive lock.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the marker cannot be written.
    pub fn mark_ready(&self) -> Result<(), BootstrapError> {
        let marker = self.marker_path();
        let file = tempfile::NamedTempFile::new_in(&self.dir)
            .map_err(|error| BootstrapError::io(&self.dir, &error))?;
        file.persist(&marker)
            .map_err(|error| BootstrapError::io(&marker, &error.error))?;
        Ok(())
    }

    /// Open (creating if needed) the lock file without truncating it.
    fn open_lock(&self) -> Result<File, BootstrapError> {
        let path = self.lock_path();
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|error| BootstrapError::io(&path, &error))
    }

    /// The lock file; outside Dylint's toolchain directory so Dylint never sees it.
    fn lock_path(&self) -> Utf8PathBuf {
        self.dir.join(format!(".{}.lock", self.toolchain))
    }

    /// The ready marker beside the lock file.
    fn marker_path(&self) -> Utf8PathBuf {
        self.dir.join(format!(".{}.ready", self.toolchain))
    }
}
