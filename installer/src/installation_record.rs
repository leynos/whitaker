//! Persistence for the staging directories of successful installations.
//!
//! The root CLI may run outside a Whitaker checkout, so it cannot always infer
//! a custom installation target from the current directory or default target.
//! This module records one absolute staging path per toolchain in Whitaker's
//! data directory. The installer owns writes; CLI adapters may read the paths
//! to configure Dylint forwarding.

use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;

use crate::dirs::BaseDirs;

const RECORD_FILENAME: &str = "installed_lint_paths.json";

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
        if self.recorded.iter().any(is_live_directory) {
            RecordContents::Current
        } else {
            RecordContents::Stale
        }
    }

    /// Consume the record and keep only the paths that still name directories.
    ///
    /// Stale entries and relative paths are ignored so a moved or deleted
    /// install cannot poison forwarding.
    #[must_use]
    pub fn into_surviving_paths(self) -> Vec<Utf8PathBuf> {
        self.recorded
            .into_iter()
            .filter(is_live_directory)
            .collect()
    }
}

/// Report whether `path` is an absolute path that still names a directory.
fn is_live_directory(path: &Utf8PathBuf) -> bool { path.is_absolute() && path.is_dir() }

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

/// Resolve the record file inside the Whitaker data directory.
fn record_path(dirs: &dyn BaseDirs) -> Result<PathBuf, InstallationRecordError> {
    dirs.whitaker_data()
        .map(|directory| directory.join(RECORD_FILENAME))
        .ok_or(InstallationRecordError::MissingDataDirectory)
}

/// Create the record file's parent directory when it is missing.
fn ensure_parent(path: &std::path::Path) -> Result<(), InstallationRecordError> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|source| record_io_error("create parent directory for", path, source))
}

/// Open the record for reading and writing, creating it when absent.
fn open_record(path: &std::path::Path) -> Result<File, InstallationRecordError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|source| record_io_error("open", path, source))
}

/// Read the recorded toolchain-to-path map from an open record file.
///
/// An empty file is treated as an empty map, so a record that was created but
/// never written reads back as "nothing installed" rather than as corruption.
fn read_paths(
    path: &std::path::Path,
    file: &mut File,
) -> Result<BTreeMap<String, String>, InstallationRecordError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|source| record_io_error("read", path, source))?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|source| record_io_error("read", path, source))?;
    if content.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str::<BTreeMap<String, String>>(&content).map_err(|source| {
        InstallationRecordError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// Replace the record's contents with `paths` and flush them to disk.
///
/// The file is truncated in place rather than replaced by a rename, so a
/// concurrent reader holding the lock sees either the old or the new record
/// and never a missing file.
fn write_paths(
    path: &std::path::Path,
    file: &mut File,
    paths: &BTreeMap<String, String>,
) -> Result<(), InstallationRecordError> {
    file.set_len(0)
        .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
        .map_err(|source| record_io_error("truncate", path, source))?;
    serde_json::to_writer(&mut *file, paths)
        .map_err(|error| record_io_error("serialize", path, std::io::Error::other(error)))?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_all())
        .map_err(|source| record_io_error("write", path, source))
}

/// Describe an operating-system failure against the record file.
///
/// The operation name is carried in the error so a caller can tell a lock
/// failure from a read or write failure without parsing the message.
fn record_io_error(
    operation: &'static str,
    path: &std::path::Path,
    source: std::io::Error,
) -> InstallationRecordError {
    InstallationRecordError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
#[path = "installation_record_tests.rs"]
mod tests;
