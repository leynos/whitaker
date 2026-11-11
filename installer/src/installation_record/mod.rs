//! Persistence for the staging directories of successful installations.
//!
//! The root CLI may run outside a Whitaker checkout, so it cannot always infer
//! a custom installation target from the current directory or default target.
//! This module records one absolute staging path per toolchain in Whitaker's
//! data directory. The installer owns writes; CLI adapters may read the paths
//! to configure Dylint forwarding.

use std::{collections::BTreeMap, fs::OpenOptions, path::PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;

use crate::dirs::BaseDirs;

const RECORD_FILENAME: &str = "installed_lint_paths.json";

mod store;
use store::{ensure_parent, open_record, read_paths, record_io_error, record_path, write_paths};

/// Errors returned when installed staging paths cannot be persisted or read.
#[derive(Debug, thiserror::Error)]
pub enum InstallationRecordError {
    /// Whitaker's platform data directory could not be resolved.
    #[error("could not determine Whitaker data directory")]
    MissingDataDirectory,

    /// The installation record is not valid JSON.
    #[error("failed to parse installation record {path}: {source}")]
    Parse {
        /// Record path.
        path: PathBuf,
        /// Underlying deserialization error.
        #[source]
        source: serde_json::Error,
    },

    /// An operation on the installation record failed.
    #[error("failed to {operation} installation record {path}: {source}")]
    Io {
        /// Operation being performed.
        operation: &'static str,
        /// Record or staging path involved in the operation.
        path: PathBuf,
        /// Underlying filesystem or serialization error.
        #[source]
        source: std::io::Error,
    },
}

/// Records the absolute staging directory for a successfully installed toolchain.
///
/// A later install for the same toolchain replaces its previous staging path;
/// paths for other toolchains remain available to Dylint's toolchain selector.
///
/// # Examples
///
/// ```no_run
/// use camino::Utf8Path;
/// use whitaker_installer::{dirs::SystemBaseDirs, installation_record::record_installation};
///
/// let dirs = SystemBaseDirs::new().ok_or("platform data directory unavailable")?;
/// record_installation(&dirs, "nightly-2025-01-01", Utf8Path::new("target"))?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an error if the staging path cannot be canonicalized or the record
/// cannot be created, locked, read, or written.
pub fn record_installation(
    dirs: &dyn BaseDirs,
    toolchain: &str,
    staging_path: &Utf8Path,
) -> Result<(), InstallationRecordError> {
    let record_path = record_path(dirs)?;
    let absolute_staging_path = staging_path.canonicalize_utf8().map_err(|source| {
        record_io_error(
            "canonicalize staging directory",
            staging_path.as_std_path(),
            source,
        )
    })?;
    ensure_parent(&record_path)?;
    let mut file = open_record(&record_path)?;
    file.lock_exclusive()
        .map_err(|source| record_io_error("lock", &record_path, source))?;
    let mut paths = match read_paths(&record_path, &mut file) {
        Ok(paths) => paths,
        Err(InstallationRecordError::Parse { path, source }) => {
            tracing::warn!(path = %path.display(), error = %source, "discarding paths");
            BTreeMap::new()
        }
        Err(error) => return Err(error),
    };
    paths.insert(toolchain.to_owned(), absolute_staging_path.into_string());
    write_paths(&record_path, &mut file, &paths)
}

/// What a read installation record says about the directories it names.
///
/// An empty path list alone cannot distinguish "nothing has been installed"
/// from "the installation has gone away", so a reader that reports on the
/// record needs this classification as well as the surviving paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordContents {
    /// No record file exists, so nothing has been installed yet.
    Missing,
    /// The record exists but stores no staging paths.
    Empty,
    /// The record stores paths, but none of them still exist.
    Stale,
    /// At least one recorded staging directory still exists.
    Current,
    /// The record stores paths, none still exists, and at least one could not
    /// be inspected.
    ///
    /// This is separate from [`Self::Stale`] because the two need different
    /// responses: a stale record means the installation has been removed,
    /// whereas an uninspectable one means the data directory could not be
    /// read, which is a fault rather than an absence.
    Uninspectable,
}

/// An installation record as read from disk, before stale entries are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationRecord {
    is_present: bool,
    recorded: Vec<Utf8PathBuf>,
}

impl InstallationRecord {
    /// A record with no file behind it.
    const fn missing() -> Self {
        Self {
            is_present: false,
            recorded: Vec::new(),
        }
    }

    /// A record read from an existing file.
    const fn present(recorded: Vec<Utf8PathBuf>) -> Self {
        Self {
            is_present: true,
            recorded,
        }
    }

    /// Classify the record against the filesystem it describes.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use whitaker_installer::{
    ///     dirs::SystemBaseDirs,
    ///     installation_record::{RecordContents, read_installation_record},
    /// };
    ///
    /// let dirs = SystemBaseDirs::new().ok_or("platform data directory unavailable")?;
    /// let record = read_installation_record(&dirs)?;
    /// assert_ne!(record.contents(), RecordContents::Stale);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn contents(&self) -> RecordContents {
        if !self.is_present {
            return RecordContents::Missing;
        }
        if self.recorded.is_empty() {
            return RecordContents::Empty;
        }
        classify_paths(self.recorded.iter().map(|path| path_state(path.as_path())))
    }

    /// Consume the record and keep only the paths that still name directories.
    ///
    /// Stale entries, uninspectable entries, and relative paths are ignored so
    /// a moved or deleted install cannot poison forwarding.
    #[must_use]
    pub fn into_surviving_paths(self) -> Vec<Utf8PathBuf> {
        self.recorded
            .into_iter()
            .filter(|path| path_state(path) == PathState::Live)
            .collect()
    }

    /// Consume the record and keep every path it names that is absolute.
    ///
    /// Existence is deliberately not tested here. A caller that inspects each
    /// path can tell a directory that has gone away from one it cannot read,
    /// and testing for a directory first would report the second case as the
    /// ordinary "nothing installed" state. Relative paths are still dropped,
    /// because a corrupt record must not point forwarding at the working
    /// directory.
    #[must_use]
    pub fn into_absolute_paths(self) -> Vec<Utf8PathBuf> {
        self.recorded
            .into_iter()
            .filter(|path| path.is_absolute())
            .collect()
    }
}

/// What a recorded path currently names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathState {
    /// The path is absolute and still names a directory.
    Live,
    /// The path is relative, names something other than a directory, or no
    /// longer exists.
    Gone,
    /// The path is absolute but could not be inspected.
    Uninspectable,
}

/// Classify one recorded path against the filesystem.
///
/// `is_dir()` folds "missing" and "could not be inspected" into the same
/// `false`, which reports a permission or I/O failure as the ordinary "nothing
/// installed" state. Reading the metadata error kind keeps those apart.
fn path_state(path: &Utf8Path) -> PathState {
    if !path.is_absolute() {
        return PathState::Gone;
    }
    match std::fs::metadata(path.as_std_path()) {
        Ok(metadata) if metadata.is_dir() => PathState::Live,
        Ok(_) => PathState::Gone,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => PathState::Gone,
        Err(_) => PathState::Uninspectable,
    }
}

/// Decide what a record's path states say about the record as a whole.
///
/// One live directory makes the record current, because Dylint selects among
/// the toolchains it is given. When none is live, a path that could not be
/// inspected outranks the stale ones: a broken data directory must not be
/// reported as an installation that has been removed.
fn classify_paths(states: impl IntoIterator<Item = PathState>) -> RecordContents {
    let mut has_uninspectable = false;
    for state in states {
        match state {
            PathState::Live => return RecordContents::Current,
            PathState::Uninspectable => has_uninspectable = true,
            PathState::Gone => {}
        }
    }
    if has_uninspectable {
        RecordContents::Uninspectable
    } else {
        RecordContents::Stale
    }
}

/// Read the installation record, keeping enough detail to classify it.
///
/// A missing record is not an error: it means nothing has been installed yet.
/// The returned record distinguishes that case from a record whose entries
/// have all gone away, which [`installed_paths`] cannot express.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::{dirs::SystemBaseDirs, installation_record::read_installation_record};
///
/// let dirs = SystemBaseDirs::new().ok_or("platform data directory unavailable")?;
/// let record = read_installation_record(&dirs)?;
/// for path in record.into_surviving_paths() {
///     println!("{}", path);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an error if the record exists but cannot be locked, read, or parsed.
pub fn read_installation_record(
    dirs: &dyn BaseDirs,
) -> Result<InstallationRecord, InstallationRecordError> {
    let record_path = record_path(dirs)?;
    let mut file = match OpenOptions::new().read(true).open(&record_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(InstallationRecord::missing());
        }
        Err(source) => return Err(record_io_error("open", &record_path, source)),
    };
    // `File::lock_shared` is stable only from Rust 1.89; naming the trait
    // keeps the 1.85-compatible implementation selected where the inherent
    // method would otherwise shadow it.
    FileExt::lock_shared(&file).map_err(|source| record_io_error("lock", &record_path, source))?;
    let recorded = read_paths(&record_path, &mut file)?
        .into_values()
        .map(Utf8PathBuf::from)
        .collect();
    Ok(InstallationRecord::present(recorded))
}

/// Returns recorded staging directories that still exist.
///
/// Missing records yield an empty vector. Stale entries and relative paths are
/// ignored so a moved or deleted install cannot poison forwarding.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::{dirs::SystemBaseDirs, installation_record::installed_paths};
///
/// let dirs = SystemBaseDirs::new().ok_or("platform data directory unavailable")?;
/// for path in installed_paths(&dirs)? {
///     println!("{}", path);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an error if the record exists but cannot be locked, read, or parsed.
pub fn installed_paths(dirs: &dyn BaseDirs) -> Result<Vec<Utf8PathBuf>, InstallationRecordError> {
    read_installation_record(dirs).map(InstallationRecord::into_surviving_paths)
}

#[cfg(test)]
mod tests;
