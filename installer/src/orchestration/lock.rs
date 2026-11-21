//! The installation lock that serializes concurrent installs and repairs.
//!
//! `whitaker check` repairs a missing installation on first use, and several
//! checks can start at the same time: an editor's lint-on-save alongside a
//! build, or a matrix of CI jobs sharing one cache. Without a lock each of
//! them would download and stage the same suite. With it, the first installs
//! and the rest queue, then find the installation ready and do nothing.

use camino::Utf8Path;
use fs2::FileExt;

use crate::{
    bootstrap::BootstrapLayout,
    dirs::BaseDirs,
    error::{InstallerError, Result},
};

/// The lock file's name inside the bootstrap directory.
const LOCK_FILE: &str = "install.lock";

/// Run `f` while holding the exclusive installation lock.
///
/// The lock lives in the bootstrap directory rather than beside the managed
/// clone, which is where a reader might expect it: the clone directory must
/// not exist before its first clone, and creating it to hold a lock would make
/// the installer treat an empty directory as a clone to update. The bootstrap
/// directory is Whitaker's own and is created by any install anyway.
///
/// The lock is advisory and is released when the file handle closes, on the
/// error path as well as the success path, so a failed install never leaves
/// the installation locked.
///
/// # Errors
///
/// Returns [`InstallerError::Bootstrap`] wrapping
/// [`BootstrapError::NoDataDirectory`](crate::bootstrap::BootstrapError::NoDataDirectory)
/// when the data directory cannot be determined, and
/// [`InstallerError::InstallLock`] when the lock file cannot be created or
/// locked. Whatever `f` returns is returned unchanged.
pub fn with_install_lock<T>(dirs: &dyn BaseDirs, f: impl FnOnce() -> T) -> Result<T> {
    let layout = BootstrapLayout::from_dirs(dirs)?;
    let path = layout.root().join(LOCK_FILE);
    let _lock = lock_file(&path)?;
    Ok(f())
}

/// Create and exclusively lock the lock file at `path`.
///
/// The handle is returned so the caller keeps it alive: closing it is what
/// releases the lock.
///
/// # Errors
///
/// Returns [`InstallerError::InstallLock`] when the containing directory
/// cannot be created, or the file cannot be opened or locked.
fn lock_file(path: &Utf8Path) -> Result<std::fs::File> {
    let directory = path.parent().unwrap_or(path);
    std::fs::create_dir_all(directory.as_std_path()).map_err(|error| lock_error(path, &error))?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.as_std_path())
        .map_err(|error| lock_error(path, &error))?;
    file.lock_exclusive()
        .map_err(|error| lock_error(path, &error))?;
    Ok(file)
}

/// Describe a lock failure against the file it concerned.
fn lock_error(path: &Utf8Path, error: &std::io::Error) -> InstallerError {
    InstallerError::InstallLock {
        path: path.to_owned(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
