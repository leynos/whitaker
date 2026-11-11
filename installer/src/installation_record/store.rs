//! Locked file access for the installation record.
//!
//! The record is a small JSON map from toolchain to staging path. These
//! helpers resolve its location, open it, and read or replace its contents
//! while the caller holds a lock. They are split from
//! [`super`] so the public record API stays readable, and so the file-level
//! concerns — truncate in place, treat an empty file as empty, name the
//! failed operation in the error — live in one place.

use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use super::InstallationRecordError;

/// Resolve the record file inside the Whitaker data directory.
pub(super) fn record_path(
    dirs: &dyn crate::dirs::BaseDirs,
) -> Result<PathBuf, InstallationRecordError> {
    dirs.whitaker_data()
        .map(|directory| directory.join(super::RECORD_FILENAME))
        .ok_or(InstallationRecordError::MissingDataDirectory)
}

/// Create the record file's parent directory when it is missing.
pub(super) fn ensure_parent(path: &Path) -> Result<(), InstallationRecordError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|source| record_io_error("create parent directory for", path, source))
}

/// Open the record for reading and writing, creating it when absent.
pub(super) fn open_record(path: &Path) -> Result<File, InstallationRecordError> {
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
pub(super) fn read_paths(
    path: &Path,
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
pub(super) fn write_paths(
    path: &Path,
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
pub(super) fn record_io_error(
    operation: &'static str,
    path: &Path,
    source: std::io::Error,
) -> InstallationRecordError {
    InstallationRecordError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
