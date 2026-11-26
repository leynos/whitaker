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
    fs2::FileExt::lock_exclusive(&file)?;
    Ok(FileLock { _file: file })
}

#[cfg(test)]
mod tests {
    //! Coverage for the capability root and the exclusive lock.

    use super::*;

    fn temp_root() -> (tempfile::TempDir, Dir) {
        let directory = match tempfile::tempdir() {
            Ok(directory) => directory,
            Err(error) => panic!("temporary directory: {error}"),
        };
        match open_root(directory.path()) {
            Ok(root) => (directory, root),
            Err(error) => panic!("open root: {error}"),
        }
    }

    #[test]
    fn open_root_opens_a_directory_and_rejects_a_missing_one() {
        let (directory, root) = temp_root();
        assert!(
            root.write("x", "data").is_ok(),
            "a root should accept a relative write"
        );
        let missing = directory.path().join("missing");
        assert_eq!(
            open_root(&missing).err().map(|error| error.kind()),
            Some(io::ErrorKind::NotFound),
            "a missing directory should be NotFound"
        );
    }

    #[test]
    fn root_methods_cover_nested_creation_listing_copy_and_read() {
        let (_directory, root) = temp_root();
        assert!(root.create_dir_all("a/b").is_ok(), "nested directories");
        assert!(root.write("a/b/file", "one").is_ok(), "write in nested dir");
        assert!(
            root.copy("a/b/file", &root, "copy").is_ok(),
            "copy within the root"
        );
        assert_eq!(
            root.read_to_string("copy").ok().as_deref(),
            Some("one"),
            "the copy should hold the same text"
        );
        let listed = match root.entries() {
            Ok(entries) => entries.filter_map(Result::ok).count(),
            Err(error) => panic!("list the root: {error}"),
        };
        assert_eq!(listed, 2, "a and copy are listed");
    }

    #[test]
    fn the_lock_excludes_a_second_holder_until_it_is_dropped() {
        let (_directory, root) = temp_root();
        let first = match lock_exclusive(&root, "lock") {
            Ok(lock) => lock,
            Err(error) => panic!("the first holder takes the lock: {error}"),
        };
        let contender = match root.open("lock") {
            Ok(file) => file.into_std(),
            Err(error) => panic!("open lock file: {error}"),
        };
        assert!(
            fs2::FileExt::try_lock_exclusive(&contender).is_err(),
            "a second holder must not get the lock while the first holds it"
        );
        drop(first);
        assert!(
            fs2::FileExt::try_lock_exclusive(&contender).is_ok(),
            "dropping the lock releases it"
        );
    }
}
