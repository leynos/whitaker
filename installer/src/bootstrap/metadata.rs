//! The relocatable record of what an installation bootstrapped.
//!
//! `bootstrap.json` names the Dylint release, Clippy snapshot, toolchain, and
//! lint-library directory one installation provisioned. Paths inside the
//! platform data root are stored relative to it and resolved at run time, so
//! moving the data root (or pointing `XDG_DATA_HOME` somewhere else) keeps a
//! working installation. Only a library directory outside the data root, such
//! as a source build's target directory, is stored absolute.

use super::error::BootstrapError;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::io::Write;

/// The metadata schema this installer writes and accepts.
pub const BOOTSTRAP_SCHEMA: u32 = 1;

/// A path recorded relative to the data root where possible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredPath {
    /// A path inside the data root, stored relative to it.
    DataRootRelative(Utf8PathBuf),
    /// A path outside the data root.
    Absolute(Utf8PathBuf),
}

impl StoredPath {
    /// Record `path`, relative to `data_root` when it lies inside it.
    ///
    /// # Examples
    ///
    /// ```
    /// use camino::Utf8Path;
    /// use whitaker_installer::bootstrap::StoredPath;
    ///
    /// let root = Utf8Path::new("/home/user/.local/share");
    /// let inside = StoredPath::new(Utf8Path::new("/home/user/.local/share/whitaker/lints"), root);
    /// assert_eq!(inside, StoredPath::DataRootRelative("whitaker/lints".into()));
    ///
    /// let moved = Utf8Path::new("/srv/share");
    /// assert_eq!(inside.resolve(moved), Utf8Path::new("/srv/share/whitaker/lints"));
    /// ```
    #[must_use]
    pub fn new(path: &Utf8Path, data_root: &Utf8Path) -> Self {
        path.strip_prefix(data_root).map_or_else(
            |_| Self::Absolute(path.to_owned()),
            |relative| Self::DataRootRelative(relative.to_owned()),
        )
    }

    /// Resolve the recorded path against the current data root.
    #[must_use]
    pub fn resolve(&self, data_root: &Utf8Path) -> Utf8PathBuf {
        match self {
            Self::DataRootRelative(relative) => data_root.join(relative),
            Self::Absolute(path) => path.clone(),
        }
    }
}

/// What one installation bootstrapped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapMetadata {
    /// The metadata schema version.
    pub schema_version: u32,
    /// The `cargo-dylint` release the driver is built with.
    pub dylint_version: String,
    /// The pinned Clippy snapshot commit.
    pub snapshot_commit: String,
    /// The nightly the lint libraries were built for.
    pub toolchain: String,
    /// The staged lint-library directory.
    pub library_dir: StoredPath,
}

impl BootstrapMetadata {
    /// Read the metadata at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::NotInstalled`] when the file is absent and
    /// [`BootstrapError::Metadata`] when it is unreadable, malformed, or from
    /// another schema.
    pub fn read(path: &Utf8Path) -> Result<Self, BootstrapError> {
        let contents = match std::fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(BootstrapError::NotInstalled {
                    path: path.to_string(),
                });
            }
            Err(error) => return Err(metadata_error(path, &error)),
        };
        let metadata: Self =
            serde_json::from_str(&contents).map_err(|error| metadata_error(path, &error))?;
        if metadata.schema_version != BOOTSTRAP_SCHEMA {
            return Err(metadata_error(
                path,
                &format!(
                    "schema {} is not supported; expected {BOOTSTRAP_SCHEMA}",
                    metadata.schema_version
                ),
            ));
        }
        Ok(metadata)
    }

    /// Persist the metadata at `path` through a temporary sibling file.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Io`] when the file cannot be written.
    pub fn write(&self, path: &Utf8Path) -> Result<(), BootstrapError> {
        let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
        std::fs::create_dir_all(parent).map_err(|error| BootstrapError::io(parent, &error))?;
        let json = serde_json::to_vec_pretty(self).map_err(|error| metadata_error(path, &error))?;
        let mut file =
            tempfile::NamedTempFile::new_in(parent).map_err(|e| BootstrapError::io(parent, &e))?;
        file.write_all(&json)
            .map_err(|e| BootstrapError::io(path, &e))?;
        file.as_file()
            .sync_all()
            .map_err(|e| BootstrapError::io(path, &e))?;
        file.persist(path)
            .map_err(|error| BootstrapError::io(path, &error.error))?;
        Ok(())
    }
}

/// Wrap a read or parse failure.
fn metadata_error(path: &Utf8Path, reason: &dyn std::fmt::Display) -> BootstrapError {
    BootstrapError::Metadata {
        path: path.to_string(),
        reason: reason.to_string(),
    }
}
