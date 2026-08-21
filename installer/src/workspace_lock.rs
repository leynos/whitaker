//! Cross-process serialization for managed Whitaker clone preparation.
//!
//! A sidecar lock prevents concurrent installers from changing the shared
//! managed checkout between action selection, ref resolution, and checkout.

use crate::error::{InstallerError, Result};
use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;
use std::fs::{File, OpenOptions};

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
        std::fs::create_dir_all(parent).map_err(|source| InstallerError::WorkspaceLock {
            path: path.clone(),
            source,
        })?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
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
    use super::{ManagedCloneLock, lock_path};
    use camino::Utf8PathBuf;
    use fs2::FileExt;
    use std::fs::OpenOptions;
    use tempfile::TempDir;

    #[test]
    fn managed_clone_lock_excludes_another_open_file() {
        let temp = TempDir::new().expect("create temporary lock directory");
        let clone_dir = Utf8PathBuf::try_from(temp.path().join("whitaker"))
            .expect("temporary lock path is UTF-8");
        let first = ManagedCloneLock::acquire(&clone_dir).expect("acquire first lock");
        let path = lock_path(&clone_dir);
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open second lock handle");

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
