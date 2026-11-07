//! Target-directory resolution and lock-path identity for the example harness.
//!
//! These helpers decide which Cargo output directory a test process belongs
//! to and derive the stable lock path that scopes coordination to it. They are
//! separate from the lock protocol so that protocol module stays within its
//! size budget.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
};

/// Resolve the Cargo output directory used by this test process.
///
/// Prefer Cargo's marker, then the executable's profile directory so
/// marker-free processes share a lock identity. Fall back to an absolute
/// override; ignore relative overrides because Cargo resolves them from its
/// invocation directory. For example, `target/debug/deps/test` falls back to
/// `target/debug`.
///
/// # Errors
///
/// Return an error if Cargo metadata cannot be read or no target path resolves.
pub(super) fn effective_target_directory(
    executable: &Path,
    configured_target_directory: Option<&Path>,
) -> io::Result<PathBuf> {
    for ancestor in executable.ancestors().skip(1) {
        match std::fs::metadata(ancestor.join(".rustc_info.json")) {
            Ok(metadata) if metadata.is_file() => return std::fs::canonicalize(ancestor),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    if let Some(profile_directory) = target_profile_directory_for_test_binary(executable) {
        return std::fs::canonicalize(profile_directory);
    }

    if let Some(target_directory) = configured_target_directory.filter(|path| path.is_absolute()) {
        return std::fs::canonicalize(target_directory);
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "could not resolve Cargo target directory from the test process",
    ))
}

/// Find the active Cargo profile directory above a test binary's `deps` path.
pub(super) fn target_profile_directory_for_test_binary(executable: &Path) -> Option<&Path> {
    let dependencies_directory = executable
        .ancestors()
        .find(|ancestor| ancestor.ends_with("deps"));
    dependencies_directory.and_then(Path::parent)
}

/// Hash one normalized target-directory path into a lock identity.
fn example_harness_lock_key(target_directory: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    target_directory.hash(&mut hasher);
    hasher.finish()
}

/// Map a target directory to its scoped lock path.
///
/// A 64-bit hash collision can only serialize unrelated targets; it cannot
/// bypass coordination for processes using the same target directory.
pub(super) fn example_harness_lock_path(target_directory: &Path, lock_root: &Path) -> PathBuf {
    lock_root.join(format!(
        "rstest-helper-example-harness-{:016x}.lock",
        example_harness_lock_key(target_directory)
    ))
}
