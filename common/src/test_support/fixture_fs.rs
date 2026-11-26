//! Capability roots for test targets.
//!
//! Test targets stage fixtures under a root (a `TempDir`, `CARGO_MANIFEST_DIR`,
//! the workspace root) whose absolute path the code under test reopens by name.
//! [`open_root`] is the one function that grants ambient authority: a test
//! calls it once for its root and then works through the returned [`Dir`] with
//! relative paths (`root.write("x", ..)`, `root.create_dir_all("a/b")`), taking
//! the absolute paths it hands to the code under test from `root_path.join(..)`.
//! Nothing here calls `std::fs` on a caller's path.

use std::{io, path::Path};

use cap_std::ambient_authority;
pub use cap_std::{fs::Dir, time::SystemTime as CapSystemTime};

/// A held cross-process file lock; dropping it releases the lock.
#[derive(Debug)]
pub struct FileLock {
    _file: std::fs::File,
}

/// Open `root` as a directory capability by ambient path, following links.
///
/// This is the single ambient grant for test fixtures. Call it once per test or
/// fixture, then use the returned [`Dir`] with relative paths.
///
/// # Errors
///
/// Returns the I/O error when `root` cannot be opened as a directory.
pub fn open_root(root: impl AsRef<Path>) -> io::Result<Dir> {
    Dir::open_ambient_dir(root.as_ref(), ambient_authority())
}

/// Open `name` inside `root` (creating it if absent) and take an exclusive,
/// cross-process lock on it, blocking until the lock is free.
///
/// Dropping the returned [`FileLock`] releases the lock.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be opened or locked.
pub fn lock_exclusive(root: &Dir, name: impl AsRef<Path>) -> io::Result<FileLock> {
    let file = root.create(name)?.into_std();
    file.lock()?;
    Ok(FileLock { _file: file })
}
