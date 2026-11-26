//! Ambient filesystem helpers for test targets.
//!
//! Test targets stage fixtures at ambient paths the code under test reopens by
//! absolute path (a `CARGO_MANIFEST_DIR` file, a `TempDir` tree, a stub
//! `cargo`), so no `Dir` capability can anchor them. This crate is already
//! exempt from the `no_std_fs_operations` policy for exactly that reason, and
//! these wrappers keep the ambient access in one named place that the test
//! targets call, instead of each target carrying its own exclusion.

use std::{fs, io, path::Path};

/// Read the whole file at `path` into a string.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be read as UTF-8 text.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    fs::read_to_string(path.as_ref())
}

/// Read the whole file at `path` into a byte vector.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be read.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> { fs::read(path.as_ref()) }

/// Write `contents` to `path`, creating or truncating the file.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be written.
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    fs::write(path.as_ref(), contents)
}

/// Create the directory at `path` and any missing ancestors.
///
/// # Errors
///
/// Returns the I/O error when a directory cannot be created.
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(path.as_ref())
}

/// Create the directory at `path`, whose parent must exist.
///
/// # Errors
///
/// Returns the I/O error when the directory cannot be created.
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> { fs::create_dir(path.as_ref()) }

/// Copy the file at `from` to `to`, returning the number of bytes copied.
///
/// # Errors
///
/// Returns the I/O error when the copy fails.
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    fs::copy(from.as_ref(), to.as_ref())
}

/// List the names of the entries directly inside the directory at `path`.
///
/// # Errors
///
/// Returns the I/O error when the directory or one of its entries cannot be read.
pub fn entry_names(path: impl AsRef<Path>) -> io::Result<Vec<String>> {
    fs::read_dir(path.as_ref())?
        .map(|entry| entry.map(|listed| listed.file_name().to_string_lossy().into_owned()))
        .collect()
}

/// Mark the file at `path` executable (mode `0o755`).
///
/// # Errors
///
/// Returns the I/O error when the permissions cannot be changed.
#[cfg(unix)]
pub fn make_executable(path: impl AsRef<Path>) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path.as_ref(), fs::Permissions::from_mode(0o755))
}

/// Return the Unix permission bits of the file at `path`.
///
/// # Errors
///
/// Returns the I/O error when the file's metadata cannot be read.
#[cfg(unix)]
pub fn mode(path: impl AsRef<Path>) -> io::Result<u32> {
    use std::os::unix::fs::PermissionsExt as _;

    Ok(fs::metadata(path.as_ref())?.permissions().mode())
}

/// Remove the directory at `path` and everything inside it.
///
/// # Errors
///
/// Returns the I/O error when the tree cannot be removed.
pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    fs::remove_dir_all(path.as_ref())
}

/// A held cross-process file lock; dropping it releases the lock.
#[derive(Debug)]
pub struct FileLock {
    _file: fs::File,
}

/// Open the file at `path` (creating it if absent) and take an exclusive,
/// cross-process lock on it, blocking until the lock is free.
///
/// Dropping the returned file releases the lock. Test targets that serialize
/// nested Cargo builds across nextest processes share a lock file at an ambient
/// path, which no `Dir` capability can anchor.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be opened or locked.
pub fn lock_exclusive(path: impl AsRef<Path>) -> io::Result<FileLock> {
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.as_ref())?;
    file.lock()?;
    Ok(FileLock { _file: file })
}

/// Create an empty file at `path`, truncating any file already there.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be created.
pub fn create_empty(path: impl AsRef<Path>) -> io::Result<()> {
    fs::File::create(path.as_ref()).map(drop)
}

/// List the paths of the entries directly inside the directory at `path`.
///
/// # Errors
///
/// Returns the I/O error when the directory or one of its entries cannot be read.
pub fn entry_paths(path: impl AsRef<Path>) -> io::Result<Vec<std::path::PathBuf>> {
    fs::read_dir(path.as_ref())?
        .map(|entry| entry.map(|listed| listed.path()))
        .collect()
}

/// Return the last-modified time of the file at `path`.
///
/// # Errors
///
/// Returns the I/O error when the metadata or its modified time cannot be read.
pub fn modified(path: impl AsRef<Path>) -> io::Result<std::time::SystemTime> {
    fs::metadata(path.as_ref())?.modified()
}

/// Set the last-modified time of the file at `path`, keeping its access time.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be opened or its times changed.
pub fn set_modified(path: impl AsRef<Path>, modified: std::time::SystemTime) -> io::Result<()> {
    let file = fs::File::options().write(true).open(path.as_ref())?;
    let times = file.metadata()?.accessed().map_or_else(
        |_| fs::FileTimes::new(),
        |accessed| fs::FileTimes::new().set_accessed(accessed),
    );
    file.set_times(times.set_modified(modified))
}
