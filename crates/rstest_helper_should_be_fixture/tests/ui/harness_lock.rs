//! Example-harness lock protocol for the UI test binary.
//!
//! `tests/ui.rs` serializes example and trybuild compilations across separate
//! nextest processes that use the same Cargo target directory. The target path
//! scopes the lock, so worktrees sharing build artefacts also share the lock.
//! This module owns the protocol and stale-owner recovery; `ui.rs` stays
//! focused on assertions rather than cross-process coordination.

use std::{
    fs::{File, OpenOptions},
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use fs2::FileExt;
use log::debug;

// A live owner keeps the OS liveness lock, so this grace period only delays
// recovery after a process has abandoned its directory.
const EXAMPLE_HARNESS_LOCK_STALE_AFTER: Duration = Duration::from_mins(5);
// Bound the default `acquire()` wait so a wedged live owner surfaces a timeout
// instead of polling forever. It exceeds the stale-recovery window so genuinely
// abandoned locks are reclaimed before this ceiling is reached.
const EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_mins(6);
const EXAMPLE_HARNESS_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(100);
const EXAMPLE_HARNESS_LOCK_OWNER_FILENAME: &str = "owner";
const EXAMPLE_HARNESS_LOCK_LIVENESS_EXTENSION: &str = "owner-lock";
const EXAMPLE_HARNESS_LOCK_LOG_TARGET: &str =
    "rstest_helper_should_be_fixture::example_harness_lock";

pub(crate) struct ExampleHarnessLock {
    path: PathBuf,
    owner: ExampleHarnessLockOwner,
    owner_liveness: File,
}

impl ExampleHarnessLock {
    /// Return the directory serializing UI fixture compilation for this target.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ExampleHarnessLockOwner(String);
impl ExampleHarnessLockOwner {
    /// Create a unique owner token for one lock acquisition.
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(format!("{}-{timestamp}-{sequence}", std::process::id()))
    }
}
impl ExampleHarnessLock {
    /// Acquire the filesystem lock for this Cargo target directory's UI fixtures.
    ///
    /// # Errors
    ///
    /// Return an error when Cargo's target directory cannot be resolved or the
    /// lock cannot be acquired before its timeout.
    pub(crate) fn acquire() -> io::Result<Self> {
        let executable = std::env::current_exe()?;
        let configured_target_directory = std::env::var_os("CARGO_TARGET_DIR");
        let target_directory = effective_target_directory(
            &executable,
            configured_target_directory.as_deref().map(Path::new),
        )?;
        Self::acquire_for_target_directory(
            &target_directory,
            &target_directory,
            Some(EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT),
        )
    }

    /// Acquire the lock selected for `target_directory` under `lock_root`.
    ///
    /// Keeping both paths injectable lets tests verify the acquisition boundary
    /// without leaving sidecar files in the host's shared temporary directory.
    fn acquire_for_target_directory(
        target_directory: &Path,
        lock_root: &Path,
        wait_limit: Option<Duration>,
    ) -> io::Result<Self> {
        let path = example_harness_lock_path(target_directory, lock_root);
        Self::acquire_at(&path, wait_limit)
    }

    /// Acquire `path`, waiting no longer than `wait_limit` when it is occupied.
    ///
    /// # Errors
    ///
    /// Return filesystem errors from lock creation and recovery, or a timeout
    /// when the current owner does not release the lock in time.
    fn acquire_at(path: &Path, wait_limit: Option<Duration>) -> io::Result<Self> {
        let started_at = Instant::now();
        let mut attempt = 0_u64;
        loop {
            attempt += 1;
            let state_guard = lock_example_harness_state(path)?;
            match create_example_harness_lock(path.to_path_buf()) {
                Ok(lock) => {
                    debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=acquired path={} owner={} attempt={} elapsed_ms={}", path.display(), lock.owner.0, attempt, started_at.elapsed().as_millis());
                    return Ok(lock);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=contended path={} attempt={} elapsed_ms={}", path.display(), attempt, started_at.elapsed().as_millis());
                    recover_stale_example_harness_lock_while_locked(path)?;
                }
                Err(error) => return Err(error),
            }
            drop(state_guard);
            wait_for_example_harness_lock_release(path, started_at, wait_limit)?;
        }
    }
}
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
fn effective_target_directory(
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
fn target_profile_directory_for_test_binary(executable: &Path) -> Option<&Path> {
    let dependencies_directory = executable
        .ancestors()
        .find(|ancestor| ancestor.ends_with("deps"));
    dependencies_directory.and_then(Path::parent)
}

/// Hash one normalized target-directory path into a lock identity.
fn example_harness_lock_key(target_directory: &Path) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    target_directory.hash(&mut hasher);
    hasher.finish()
}

/// Map a target directory to its scoped lock path.
///
/// A 64-bit hash collision can only serialize unrelated targets; it cannot
/// bypass coordination for processes using the same target directory.
fn example_harness_lock_path(target_directory: &Path, lock_root: &Path) -> PathBuf {
    lock_root.join(format!(
        "rstest-helper-example-harness-{:016x}.lock",
        example_harness_lock_key(target_directory)
    ))
}

/// Create the lock directory, returning `AlreadyExists` for an active owner.
fn create_example_harness_lock(path: PathBuf) -> io::Result<ExampleHarnessLock> {
    let Some(owner_liveness) = try_lock_example_harness_liveness(&path)? else {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    };
    std::fs::create_dir(&path)?;
    let owner = ExampleHarnessLockOwner::new();
    if let Err(error) = write_lock_owner(&path, &owner) {
        // Best-effort rollback; the original write failure is the useful one.
        let _rollback_result = remove_example_harness_lock_directory(&path);
        return Err(error);
    }
    Ok(ExampleHarnessLock {
        path,
        owner,
        owner_liveness,
    })
}
impl Drop for ExampleHarnessLock {
    /// Release the owned lock directory and its OS liveness lock.
    fn drop(&mut self) {
        if let Ok(_state_guard) = lock_example_harness_state(&self.path) {
            // Best-effort cleanup: a failure only leaves a directory to reclaim.
            let _cleanup_result = remove_lock_if_owned(&self.path, &self.owner);
        }
        // Release liveness after owner-aware cleanup to avoid successor races.
        let _unlock_result = FileExt::unlock(&self.owner_liveness);
    }
}

/// Recover stale state and wait once before another acquisition attempt.
///
/// # Errors
///
/// Return recovery errors or a timeout when the lock remains occupied.
fn wait_for_example_harness_lock_release(
    path: &Path,
    started_at: Instant,
    wait_limit: Option<Duration>,
) -> io::Result<()> {
    recover_stale_example_harness_lock(path)?;
    if wait_limit.is_some_and(|limit| started_at.elapsed() >= limit) {
        debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=timed_out path={} elapsed_ms={}", path.display(), started_at.elapsed().as_millis());
        return Err(example_harness_lock_timeout(path));
    }
    std::thread::sleep(EXAMPLE_HARNESS_LOCK_POLL_INTERVAL);
    Ok(())
}

/// Create the timeout error for a still-owned lock path.
fn example_harness_lock_timeout(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "timed out waiting for example harness lock at {}",
            path.display()
        ),
    )
}
/// Recover a stale lock while holding its state sidecar.
///
/// # Errors
///
/// Return errors from state-lock acquisition or stale-owner recovery.
fn recover_stale_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    recover_stale_example_harness_lock_while_locked(path)
}
/// Inspect and remove a stale directory while its state sidecar is locked.
///
/// # Errors
///
/// Return filesystem or liveness-lock errors encountered during recovery.
fn recover_stale_example_harness_lock_while_locked(path: &Path) -> io::Result<()> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let modified = metadata.modified()?;

    if example_harness_lock_is_stale(modified, SystemTime::now()) {
        debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=stale_recovery_eligible path={}", path.display());
        remove_stale_example_harness_lock_while_locked(path)?;
    }
    Ok(())
}
/// Remove a released stale lock while holding its state sidecar.
///
/// # Errors
///
/// Return errors from state locking, liveness checks, or directory removal.
fn remove_stale_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    remove_stale_example_harness_lock_while_locked(path)
}
/// Remove a stale directory only when no live owner holds its liveness lock.
///
/// # Errors
///
/// Return filesystem errors encountered while checking or removing the owner.
fn remove_stale_example_harness_lock_while_locked(path: &Path) -> io::Result<()> {
    let Some(_owner_liveness) = try_lock_example_harness_liveness(path)? else {
        debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=stale_recovery_live_owner path={}", path.display());
        return Ok(());
    };
    read_example_harness_lock_owner(path)?.map_or_else(
        || remove_example_harness_lock_directory(path),
        |owner| remove_lock_if_owned(path, &owner),
    )
}
/// Lock the persistent sidecar that serializes owner-state changes.
///
/// # Errors
///
/// Return errors from opening or locking the state sidecar.
fn lock_example_harness_state(path: &Path) -> io::Result<File> {
    let state_path = path.with_extension("state");
    let state_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(state_path)?;
    state_file.lock_exclusive()?;
    Ok(state_file)
}
/// Acquire the OS lock that distinguishes live owners from abandoned ones.
///
/// # Errors
///
/// Return errors other than the platform's ordinary lock-contention result.
fn try_lock_example_harness_liveness(path: &Path) -> io::Result<Option<File>> {
    let liveness_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path.with_extension(EXAMPLE_HARNESS_LOCK_LIVENESS_EXTENSION))?;
    match liveness_file.try_lock_exclusive() {
        Ok(()) => Ok(Some(liveness_file)),
        Err(error) if example_harness_liveness_lock_is_contended(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Recognize the platform error returned for a contended advisory file lock.
fn example_harness_liveness_lock_is_contended(error: &io::Error) -> bool {
    error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
}

/// Write the owner token into the acquired lock directory.
///
/// # Errors
///
/// Return an error when the owner file cannot be written.
fn write_lock_owner(path: &Path, owner: &ExampleHarnessLockOwner) -> io::Result<()> {
    std::fs::write(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME), &owner.0)
}

/// Read the current owner token when its metadata file exists.
///
/// # Errors
///
/// Return an error when owner metadata exists but cannot be read.
fn read_example_harness_lock_owner(path: &Path) -> io::Result<Option<ExampleHarnessLockOwner>> {
    match std::fs::read_to_string(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME)) {
        Ok(owner) => Ok(Some(ExampleHarnessLockOwner(owner))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Remove the lock directory only when `owner` still owns it.
///
/// # Errors
///
/// Return filesystem errors from reading owner metadata or removing the lock.
fn remove_lock_if_owned(path: &Path, owner: &ExampleHarnessLockOwner) -> io::Result<()> {
    let Some(current_owner) = read_example_harness_lock_owner(path)? else {
        return Ok(());
    };
    if current_owner != *owner {
        debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=owner_mismatch path={} expected_owner={} actual_owner={}", path.display(), owner.0, current_owner.0);
        return Ok(());
    }
    remove_example_harness_lock_directory(path)
}

/// Remove the owner file and lock directory, treating an absent path as released.
///
/// # Errors
///
/// Return errors other than an already-removed owner file or lock directory.
fn remove_example_harness_lock_directory(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME)) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    match std::fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Classify a lock as stale only after its age exceeds the configured grace period.
fn example_harness_lock_is_stale(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .is_ok_and(|age| age > EXAMPLE_HARNESS_LOCK_STALE_AFTER)
}

#[cfg(test)]
#[path = "harness_lock_tests.rs"]
mod tests;
