//! Capability-based filesystem helpers for test targets.
//!
//! Test targets stage fixtures at paths the code under test reopens by absolute
//! path (a `CARGO_MANIFEST_DIR` file, a `TempDir` tree, a stub `cargo`). Each
//! helper here anchors a [`Dir`] on the path's parent once, through `anchor`,
//! the one place the ambient authority is granted, and does the work through
//! that `Dir`. Nothing here calls `std::fs`.

use std::{
    io,
    path::{Path, PathBuf},
};

use cap_std::{ambient_authority, fs::Dir};

/// A held cross-process file lock; dropping it releases the lock.
#[derive(Debug)]
pub struct FileLock {
    _file: std::fs::File,
}

/// Open `path` as a directory by ambient path, following links.
///
/// This is the single call that grants ambient authority to the helpers in
/// this module; every other operation goes through the `Dir` it returns.
fn open_dir(path: &Path) -> io::Result<Dir> { Dir::open_ambient_dir(path, ambient_authority()) }

/// Anchor on the parent of `path` and return it with the entry name.
fn anchor(path: &Path) -> io::Result<(Dir, std::ffi::OsString)> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("path should name an entry"))?
        .to_owned();
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    Ok((open_dir(parent)?, name))
}

/// Read the whole file at `path` into a string.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be read as UTF-8 text.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let (dir, name) = anchor(path.as_ref())?;
    dir.read_to_string(name)
}

/// Read the whole file at `path` into a byte vector.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be read.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let (dir, name) = anchor(path.as_ref())?;
    dir.read(name)
}

/// Write `contents` to `path`, creating or truncating the file.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be written.
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let (dir, name) = anchor(path.as_ref())?;
    dir.write(name, contents)
}

/// Create an empty file at `path`, truncating any file already there.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be created.
pub fn create_empty(path: impl AsRef<Path>) -> io::Result<()> { write(path, []) }

/// Create the directory at `path`, whose parent must exist.
///
/// # Errors
///
/// Returns the I/O error when the directory cannot be created.
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    let (dir, name) = anchor(path.as_ref())?;
    dir.create_dir(name)
}

/// Create the directory at `path` and any missing ancestors.
///
/// Each missing component is created inside its parent, opened by path, so a
/// path with `..` or a directory symlink resolves as it does for `std::fs`.
///
/// # Errors
///
/// Returns the I/O error when a directory cannot be created.
pub fn create_dir_all(target: impl AsRef<Path>) -> io::Result<()> {
    let path = target.as_ref();
    if path.as_os_str().is_empty() || open_dir(path).is_ok() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    if path.file_name().is_none() {
        return open_dir(path).map(drop);
    }
    match create_dir(path) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            open_dir(path).map(drop).map_err(|_| error)
        }
        other => other,
    }
}

/// Remove the directory at `path` and everything inside it.
///
/// # Errors
///
/// Returns the I/O error when the tree cannot be removed.
pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let (dir, name) = anchor(path.as_ref())?;
    dir.remove_dir_all(name)
}

/// Copy the file at `from` to `to`, returning the number of bytes copied.
///
/// # Errors
///
/// Returns the I/O error when the copy fails.
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    let (from_dir, from_name) = anchor(from.as_ref())?;
    let (to_dir, to_name) = anchor(to.as_ref())?;
    from_dir.copy(from_name, &to_dir, to_name)
}

/// List the names of the entries directly inside the directory at `path`.
///
/// # Errors
///
/// Returns the I/O error when the directory or one of its entries cannot be read.
pub fn entry_names(path: impl AsRef<Path>) -> io::Result<Vec<String>> {
    open_dir(path.as_ref())?
        .entries()?
        .map(|entry| entry.map(|listed| listed.file_name().to_string_lossy().into_owned()))
        .collect()
}

/// List the paths of the entries directly inside the directory at `path`.
///
/// # Errors
///
/// Returns the I/O error when the directory or one of its entries cannot be read.
pub fn entry_paths(directory: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let path = directory.as_ref();
    open_dir(path)?
        .entries()?
        .map(|entry| entry.map(|listed| path.join(listed.file_name())))
        .collect()
}

/// Mark the file at `path` executable (mode `0o755`).
///
/// # Errors
///
/// Returns the I/O error when the permissions cannot be changed.
#[cfg(unix)]
pub fn make_executable(path: impl AsRef<Path>) -> io::Result<()> {
    use cap_std::fs::{Permissions, PermissionsExt as _};

    let (dir, name) = anchor(path.as_ref())?;
    dir.set_permissions(name, Permissions::from_mode(0o755))
}

/// Return the Unix permission bits of the file at `path`.
///
/// # Errors
///
/// Returns the I/O error when the file's metadata cannot be read.
#[cfg(unix)]
pub fn mode(path: impl AsRef<Path>) -> io::Result<u32> {
    use cap_std::fs::PermissionsExt as _;

    let (dir, name) = anchor(path.as_ref())?;
    Ok(dir.metadata(name)?.permissions().mode())
}

/// Return the last-modified time of the file at `path`.
///
/// # Errors
///
/// Returns the I/O error when the metadata or its modified time cannot be read.
pub fn modified(path: impl AsRef<Path>) -> io::Result<std::time::SystemTime> {
    let (dir, name) = anchor(path.as_ref())?;
    dir.metadata(name)?
        .modified()
        .map(cap_std::time::SystemTime::into_std)
}

/// Open the file at `path` (creating it if absent) and take an exclusive,
/// cross-process lock on it, blocking until the lock is free.
///
/// Dropping the returned [`FileLock`] releases the lock.
///
/// # Errors
///
/// Returns the I/O error when the file cannot be opened or locked.
pub fn lock_exclusive(path: impl AsRef<Path>) -> io::Result<FileLock> {
    let (dir, name) = anchor(path.as_ref())?;
    let file = dir.create(name)?.into_std();
    file.lock()?;
    Ok(FileLock { _file: file })
}
