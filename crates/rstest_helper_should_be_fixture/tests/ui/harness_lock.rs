//! Example-harness lock protocol for the UI test binary.
//!
//! `tests/ui.rs` serializes example and trybuild compilations that share a
//! Cargo `deps` directory across separate nextest processes. The temporary lock
//! is keyed by a hash of the test executable's `deps` directory, so independent
//! worktrees normally do not block one another. A hash collision shares a lock
//! and only adds serialization; it cannot permit concurrent use. This module
//! owns the lock protocol, while its adapter tests live in a child module so
//! `ui.rs` stays focused on fixture assertions.

use std::{
    collections::hash_map::DefaultHasher,
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
    /// Return the lock directory serializing this UI fixture compilation.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ExampleHarnessLockOwner(String);
impl ExampleHarnessLockOwner {
    /// Create a unique token identifying this process's lock ownership.
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
    /// Acquire the lock shared by processes using the current Cargo target.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let lock = ExampleHarnessLock::acquire()?;
    /// assert!(lock.path().is_dir());
    /// // The lock remains held until `lock` is dropped.
    /// # Ok::<(), std::io::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error when the executable path cannot be resolved or the lock
    /// cannot be acquired.
    pub(crate) fn acquire() -> io::Result<Self> {
        let executable = std::env::current_exe()?;
        let target_deps_directory = executable.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "test executable has no parent directory",
            )
        })?;
        let lock_path = example_harness_lock_path(target_deps_directory);
        Self::acquire_at(&lock_path, Some(EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT))
    }

    /// Acquire the lock at `path`, waiting no longer than the optional limit.
    ///
    /// # Errors
    /// Returns an error if lock state cannot be read, written, acquired, or
    /// reclaimed before the wait limit expires.
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

/// Return a temporary lock path for the target dependencies directory.
///
/// # Examples
///
/// ```ignore
/// let deps = std::path::Path::new("target/debug/deps");
/// let lock_path = example_harness_lock_path(deps);
/// assert!(lock_path.starts_with(std::env::temp_dir()));
/// assert_eq!(lock_path.extension().and_then(|ext| ext.to_str()), Some("lock"));
/// ```
fn example_harness_lock_path(target_deps_directory: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    target_deps_directory.hash(&mut hasher);
    example_harness_lock_path_for_digest(hasher.finish())
}

/// Map a target-directory digest to its shared lock filename.
///
/// Keep this helper private to target-path mapping so tests can model digest
/// collisions without changing the production hashing policy.
fn example_harness_lock_path_for_digest(digest: u64) -> PathBuf {
    std::env::temp_dir().join(format!("rstest-helper-example-harness-{digest:016x}.lock"))
}

/// Create the lock directory and record the token that owns it.
///
/// # Errors
/// Returns an error if the liveness lock, directory, or owner record cannot be
/// created.
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
    /// Remove the lock directory when this guard still owns it.
    fn drop(&mut self) {
        if let Ok(_state_guard) = lock_example_harness_state(&self.path) {
            // Best-effort cleanup: a failure only leaves a directory to reclaim.
            let _cleanup_result = remove_lock_if_owned(&self.path, &self.owner);
        }
        // Release liveness after owner-aware cleanup to avoid successor races.
        let _unlock_result = FileExt::unlock(&self.owner_liveness);
    }
}

/// Wait for release or return when the configured wait limit expires.
///
/// # Errors
/// Returns an error if stale-lock recovery fails or the wait limit expires.
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

/// Build the timeout error for a lock at the supplied path.
fn example_harness_lock_timeout(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "timed out waiting for example harness lock at {}",
            path.display()
        ),
    )
}
/// Recover an eligible stale lock while holding the state-transition lock.
///
/// # Errors
/// Returns an error if lock metadata cannot be read or stale cleanup fails.
fn recover_stale_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    recover_stale_example_harness_lock_while_locked(path)
}
/// Recover an eligible stale lock when the caller already holds its state lock.
///
/// # Errors
/// Returns an error if lock metadata cannot be read or stale cleanup fails.
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

/// Determine whether a directory is older than the stale-lock threshold.
fn example_harness_lock_is_stale(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .is_ok_and(|age| age > EXAMPLE_HARNESS_LOCK_STALE_AFTER)
}

/// Reclaim an eligible stale lock under its state-transition lock.
///
/// # Errors
/// Returns an error if lock state cannot be opened or stale cleanup fails.
fn remove_stale_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    remove_stale_example_harness_lock_while_locked(path)
}
/// Reclaim an eligible stale lock when its state lock is already held.
///
/// # Errors
/// Returns an error if owner metadata cannot be read or cleanup fails.
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
/// Lock the sidecar that serializes ownership changes for a lock path.
///
/// # Errors
/// Returns an error if the sidecar cannot be opened or exclusively locked.
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
/// Try to hold the sidecar that marks a live lock owner.
///
/// # Errors
/// Returns an error if the sidecar cannot be opened or locked for a reason
/// other than another live owner.
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

/// Check whether an advisory-lock error means another process owns the lock.
fn example_harness_liveness_lock_is_contended(error: &io::Error) -> bool {
    error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
}

/// Write the owner token into the lock directory.
///
/// # Errors
/// Returns an error if the owner file cannot be written.
fn write_lock_owner(path: &Path, owner: &ExampleHarnessLockOwner) -> io::Result<()> {
    std::fs::write(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME), &owner.0)
}

/// Read the owner token, returning `None` when the owner file is absent.
///
/// # Errors
/// Returns an error if the owner file exists but cannot be read.
fn read_example_harness_lock_owner(path: &Path) -> io::Result<Option<ExampleHarnessLockOwner>> {
    match std::fs::read_to_string(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME)) {
        Ok(owner) => Ok(Some(ExampleHarnessLockOwner(owner))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Remove the lock directory only when its recorded owner matches `owner`.
///
/// # Errors
/// Returns an error if owner metadata cannot be read or the directory cannot
/// be removed.
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

/// Remove owner metadata and then remove the lock directory.
///
/// # Errors
/// Returns an error if filesystem cleanup fails for a reason other than a
/// missing owner file or directory.
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

#[cfg(test)]
#[path = "harness_lock_tests.rs"]
mod tests;
