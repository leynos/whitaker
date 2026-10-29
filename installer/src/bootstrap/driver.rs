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
use super::probe::metadata_if_present;
use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;
use std::fs::File;

/// The driver Dylint persists inside its toolchain directory.
///
/// Extensionless on every platform: Dylint 6.3.0's `driver_builder::get`
/// persists the built binary to `driver_dir.join("dylint-driver")`, Windows
/// included; only the intermediate build output carries `.exe`.
const DRIVER_FILE: &str = "dylint-driver";

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

    /// The toolchain channel whose driver this slot holds.
    #[must_use]
    pub fn toolchain(&self) -> &str {
        &self.toolchain
    }

    /// The directory exported as `DYLINT_DRIVER_PATH`.
    #[must_use]
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    /// Return whether a complete driver is recorded as ready.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the slot cannot be read for any
    /// reason other than not existing yet.
    pub fn is_ready(&self) -> Result<bool, BootstrapError> {
        let is_marked = metadata_if_present(&self.marker_path())?.is_some_and(|m| m.is_file());
        Ok(is_marked && self.has_driver()?)
    }

    /// Return whether Dylint has written a driver for this toolchain.
    ///
    /// Dylint names the toolchain directory after the library's toolchain,
    /// which carries the host triple (`nightly-2026-05-28-x86_64-...`), so any
    /// directory for this channel counts. Dylint persists the driver with a
    /// rename, so a present, non-empty file is a complete one.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the slot cannot be read for any
    /// reason other than not existing yet.
    pub fn has_driver(&self) -> Result<bool, BootstrapError> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(BootstrapError::io(&self.dir, &error)),
        };
        for entry in entries {
            let entry = entry.map_err(|error| BootstrapError::io(&self.dir, &error))?;
            // A name that is not UTF-8 cannot be Dylint's toolchain directory.
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if self.is_toolchain_dir(&name) && self.holds_driver(&name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Return whether `name` is a toolchain directory for this channel.
    fn is_toolchain_dir(&self, name: &str) -> bool {
        name.strip_prefix(&self.toolchain)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
    }

    /// Return whether the toolchain directory `name` holds a non-empty driver.
    fn holds_driver(&self, name: &str) -> Result<bool, BootstrapError> {
        let driver = self.dir.join(name).join(DRIVER_FILE);
        Ok(metadata_if_present(&driver)?.is_some_and(|m| m.is_file() && m.len() > 0))
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
