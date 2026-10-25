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
    let record_path = record_path(dirs)?;
    let mut file = match OpenOptions::new().read(true).open(&record_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(record_io_error("open", &record_path, source)),
    };
    file.lock_shared()
        .map_err(|source| record_io_error("lock", &record_path, source))?;
    let paths = read_paths(&record_path, &mut file)?;
    Ok(paths
        .into_values()
        .map(Utf8PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_dir())
        .collect())
}

fn record_path(dirs: &dyn BaseDirs) -> Result<PathBuf, InstallationRecordError> {
    dirs.whitaker_data_dir()
        .map(|directory| directory.join(RECORD_FILENAME))
        .ok_or(InstallationRecordError::MissingDataDirectory)
}

fn ensure_parent(path: &std::path::Path) -> Result<(), InstallationRecordError> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|source| record_io_error("create parent directory for", path, source))
}

fn open_record(path: &std::path::Path) -> Result<File, InstallationRecordError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|source| record_io_error("open", path, source))
}

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
mod tests {
    //! Tests for exact installed-path persistence.

    use std::path::PathBuf;

    use camino::Utf8Path;
    use tempfile::TempDir;

    use super::{InstallationRecordError, installed_paths, record_installation};
    use crate::dirs::{BaseDirs, MockBaseDirs};

    fn mock_dirs(data: PathBuf) -> MockBaseDirs {
        let mut dirs = MockBaseDirs::new();
        dirs.expect_whitaker_data_dir().return_const(Some(data));
        dirs
    }

    #[test]
    fn installation_paths_preserve_toolchains_and_replace_the_same_channel() {
        let temporary = TempDir::new().expect("temporary directory should be created");
        let stable = temporary.path().join("stable");
        let nightly = temporary.path().join("nightly");
        let replacement = temporary.path().join("stable-replacement");
        std::fs::create_dir_all(&stable).expect("stable staging directory should be created");
        std::fs::create_dir_all(&nightly).expect("nightly staging directory should be created");
        std::fs::create_dir_all(&replacement)
            .expect("replacement staging directory should be created");
        let expected_nightly = Utf8Path::from_path(&nightly)
            .expect("nightly path should be UTF-8")
            .canonicalize_utf8()
            .expect("nightly path should be canonicalized");
        let expected_replacement = Utf8Path::from_path(&replacement)
            .expect("replacement path should be UTF-8")
            .canonicalize_utf8()
            .expect("replacement path should be canonicalized");
        let dirs = mock_dirs(temporary.path().join("data"));

        record_installation(
            &dirs,
            "stable",
            Utf8Path::from_path(&stable).expect("stable path should be UTF-8"),
        )
        .expect("stable path should be recorded");
        record_installation(
            &dirs,
            "nightly",
            Utf8Path::from_path(&nightly).expect("nightly path should be UTF-8"),
        )
        .expect("nightly path should be recorded");
        record_installation(
            &dirs,
            "stable",
            Utf8Path::from_path(&replacement).expect("replacement path should be UTF-8"),
        )
        .expect("replacement path should be recorded");

        let recorded = installed_paths(&dirs).expect("recorded paths should load");
        assert_eq!(recorded.len(), 2);
        assert!(recorded.contains(&expected_replacement));
        assert!(recorded.contains(&expected_nightly));
    }

    #[test]
    fn installed_paths_ignore_stale_records_and_missing_files() {
        let temporary = TempDir::new().expect("temporary directory should be created");
        let staging = temporary.path().join("staging");
        std::fs::create_dir_all(&staging).expect("staging directory should be created");
        let dirs = mock_dirs(temporary.path().join("data"));
        assert!(
            installed_paths(&dirs)
                .expect("a missing record should be an empty inventory")
                .is_empty()
        );
        record_installation(
            &dirs,
            "gone",
            Utf8Path::from_path(&staging).expect("stale path should be UTF-8"),
        )
        .expect("existing staging directory should be recorded");
        std::fs::remove_dir(&staging).expect("staging directory should be removed");
        assert!(
            installed_paths(&dirs)
                .expect("stale records should be ignored")
                .is_empty()
        );
    }

    #[test]
    fn successful_install_replaces_a_malformed_record() {
        let temporary = TempDir::new().expect("temporary directory should be created");
        let data = temporary.path().join("data");
        let staging = temporary.path().join("staging");
        std::fs::create_dir_all(&data).expect("data directory should be created");
        std::fs::create_dir_all(&staging).expect("staging directory should be created");
        std::fs::write(data.join(super::RECORD_FILENAME), "not JSON")
            .expect("malformed record should be written");
        let dirs = mock_dirs(data);

        assert!(matches!(
            installed_paths(&dirs),
            Err(InstallationRecordError::Parse { .. })
        ));
        record_installation(
            &dirs,
            "nightly",
            Utf8Path::from_path(&staging).expect("staging path should be UTF-8"),
        )
        .expect("successful install should replace malformed record");

        assert_eq!(
            installed_paths(&dirs)
                .expect("new record should load")
                .len(),
            1
        );
    }

    #[test]
    fn the_record_api_uses_only_the_whitaker_data_directory() {
        let temporary = TempDir::new().expect("temporary directory should be created");
        let staging = temporary.path().join("staging");
        std::fs::create_dir_all(&staging).expect("staging directory should be created");
        let dirs = mock_dirs(temporary.path().join("data"));

        record_installation(
            &dirs,
            "nightly",
            Utf8Path::from_path(&staging).expect("staging path should be UTF-8"),
        )
        .expect("staging path should be recorded");

        let data_dir = dirs
            .whitaker_data_dir()
            .expect("data directory should exist");
        assert!(data_dir.join("installed_lint_paths.json").is_file());
    }
}
