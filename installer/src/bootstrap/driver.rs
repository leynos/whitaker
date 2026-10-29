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
use std::time::SystemTime;

/// The driver Dylint persists inside its toolchain directory.
///
/// Extensionless on every platform: Dylint 6.3.0's `driver_builder::get`
/// persists the built binary to `driver_dir.join("dylint-driver")`, Windows
/// included; only the intermediate build output carries `.exe`.
const DRIVER_FILE: &str = "dylint-driver";

/// What distinguishes one persisted driver file from its replacement.
///
/// On Unix the device and inode change with every rename into place. On
/// Windows the creation time, kept to 100 ns, belongs to the new file. The
/// length and modification time are kept as well, as a backstop on
/// filesystems that report no creation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverIdentity {
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    file: (u64, u64),
}

impl DriverIdentity {
    /// Capture the identity of the file described by `metadata`.
    fn of(metadata: &std::fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            #[cfg(unix)]
            file: {
                use std::os::unix::fs::MetadataExt;
                (metadata.dev(), metadata.ino())
            },
        }
    }
}

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
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the slot cannot be read for any
    /// reason other than not existing yet.
    pub fn has_driver(&self) -> Result<bool, BootstrapError> {
        Ok(self.driver_metadata()?.is_some())
    }

    /// Identify the driver file now present, if any.
    ///
    /// Dylint persists a driver by renaming a new file into place, so a
    /// rebuild changes the identity even when it writes identical bytes. That
    /// makes the identity an independent signal of an actual build, whichever
    /// path the run took to reach the driver.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the slot cannot be read for any
    /// reason other than not existing yet.
    pub fn driver_identity(&self) -> Result<Option<DriverIdentity>, BootstrapError> {
        Ok(self
            .driver_metadata()?
            .map(|metadata| DriverIdentity::of(&metadata)))
    }

    /// Return the metadata of this channel's complete driver, if one exists.
    ///
    /// Dylint 6.3.0 keeps the driver at exactly
    /// `DYLINT_DRIVER_PATH/<toolchain>/dylint-driver`, keyed by the toolchain in
    /// the lint libraries' file names (`lib<crate>@<toolchain>`), which is the
    /// channel this slot records. Looking anywhere else could describe a driver
    /// Dylint never selects. Dylint persists with a rename, so a present,
    /// non-empty file is complete; an empty one is not a driver.
    fn driver_metadata(&self) -> Result<Option<std::fs::Metadata>, BootstrapError> {
        let driver = self.driver_path();
        Ok(metadata_if_present(&driver)?.filter(|m| m.is_file() && m.len() > 0))
    }

    /// The driver file Dylint builds and loads for this toolchain.
    fn driver_path(&self) -> Utf8PathBuf {
        self.dir.join(&self.toolchain).join(DRIVER_FILE)
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

    /// Forget that the driver is ready, so the next run takes the cold path.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when an existing marker cannot be removed.
    pub fn clear_ready(&self) -> Result<(), BootstrapError> {
        remove_if_present(&self.marker_path())
    }

    /// Remove the driver Dylint built, so the next build starts afresh. Call
    /// only under the exclusive lock.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when an existing driver cannot be removed.
    pub fn remove_driver(&self) -> Result<(), BootstrapError> {
        remove_if_present(&self.driver_path())
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

/// Remove the file at `path`, treating its absence as success.
fn remove_if_present(path: &Utf8Path) -> Result<(), BootstrapError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BootstrapError::io(path, &error)),
    }
}
