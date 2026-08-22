//! Cross-process serialization for managed Whitaker clone preparation.
//!
//! A sidecar lock prevents concurrent installers from changing the shared
//! managed checkout between action selection, ref resolution, and checkout.

use crate::error::{InstallerError, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, OpenOptions},
};
use fs2::FileExt;
use std::fs::File;

/// An exclusive advisory lock held while preparing the managed clone.
pub(crate) struct ManagedCloneLock {
    _file: File,
}

impl ManagedCloneLock {
    /// Acquires the sidecar lock for `clone_dir`, waiting for another installer
    /// to finish preparation before re-evaluating the workspace action.
    pub(crate) fn acquire(clone_dir: &Utf8Path) -> Result<Self> {
        let path = lock_path(clone_dir);
        let parent = path
            .parent()
            .ok_or_else(|| InstallerError::WorkspaceNotFound {
                reason: format!("could not determine parent directory for workspace lock {path}"),
            })?;
        let file_name = path
            .file_name()
            .ok_or_else(|| InstallerError::WorkspaceNotFound {
                reason: format!("could not determine file name for workspace lock {path}"),
            })?;
        Dir::create_ambient_dir_all(parent, ambient_authority()).map_err(|source| {
            InstallerError::WorkspaceLock {
                path: path.clone(),
                source,
            }
        })?;
        let directory = Dir::open_ambient_dir(parent, ambient_authority()).map_err(|source| {
            InstallerError::WorkspaceLock {
                path: path.clone(),
                source,
            }
        })?;
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        let file = directory
            .open_with(file_name, &options)
            .map(|file| file.into_std())
            .map_err(|source| InstallerError::WorkspaceLock {
                path: path.clone(),
                source,
            })?;
        file.lock_exclusive()
            .map_err(|source| InstallerError::WorkspaceLock { path, source })?;
        Ok(Self { _file: file })
    }
}

/// Returns the persistent sidecar lock path for a managed clone directory.
#[must_use]
pub(crate) fn lock_path(clone_dir: &Utf8Path) -> Utf8PathBuf {
    clone_dir.with_extension("lock")
}

#[cfg(test)]
mod tests {
    //! Validates capability-scoped managed-clone lock access and exclusion.

    use super::ManagedCloneLock;
    use camino::Utf8PathBuf;
    use cap_std::{
        ambient_authority,
        fs_utf8::{Dir, OpenOptions},
    };
    use fs2::FileExt;
    use tempfile::TempDir;

    #[test]
    fn managed_clone_lock_excludes_another_open_file() {
        let temp = TempDir::new().expect("create temporary lock directory");
        let clone_dir = Utf8PathBuf::try_from(temp.path().join("whitaker"))
            .expect("temporary lock path is UTF-8");
        let first = ManagedCloneLock::acquire(&clone_dir).expect("acquire first lock");
        let parent = clone_dir
            .parent()
            .expect("temporary clone directory has a parent");
        let directory = Dir::open_ambient_dir(parent, ambient_authority())
            .expect("open temporary lock directory capability");
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        let second = directory
            .open_with("whitaker.lock", &options)
            .expect("open second lock handle through directory capability")
            .into_std();

        let error = second
            .try_lock_exclusive()
            .expect_err("second handle must contend for the lock");
        assert_eq!(error.kind(), fs2::lock_contended_error().kind());

        drop(first);
        second
            .try_lock_exclusive()
            .expect("second handle acquires lock after release");
    }
}
