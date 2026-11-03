//! Example-harness lock protocol for the UI test binary.
//!
//! `tests/ui.rs`'s `ExampleHarness` fixture serializes example compilations
//! across separate nextest processes with a filesystem lock. This module owns
//! that protocol — the lock type and its stale-owner recovery — so `ui.rs`
//! stays focused on the example and trybuild assertions. Adapter tests live
//! beside it in `harness_lock_tests.rs`.

use std::{
    fs::{File, OpenOptions},
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
    pub(crate) fn path(&self) -> &Path { &self.path }
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
    /// The lock lives inside the resolved target directory, so worktrees that
    /// share build artefacts serialize against each other, while independent
    /// target directories never contend. A host-global path would instead let
    /// one worktree's build block another's for the whole stale window.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo's target directory cannot be resolved or
    /// the lock cannot be acquired before its timeout.
    pub(crate) fn acquire() -> io::Result<Self> {
        let executable = std::env::current_exe()?;
        let configured_target_directory = std::env::var_os("CARGO_TARGET_DIR");
        let target_directory = effective_target_directory(
            &executable,
            configured_target_directory.as_deref().map(Path::new),
        )?;
        Self::acquire_at(
            &target_directory.join("rstest-helper-example-harness.lock"),
            Some(EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT),
        )
    }

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
    fn drop(&mut self) {
        if let Ok(_state_guard) = lock_example_harness_state(&self.path) {
            // Best-effort cleanup: a failure only leaves a directory to reclaim.
            let _cleanup_result = remove_lock_if_owned(&self.path, &self.owner);
        }
        // Release liveness after owner-aware cleanup to avoid successor races.
        let _unlock_result = FileExt::unlock(&self.owner_liveness);
    }
}

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

fn example_harness_lock_timeout(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "timed out waiting for example harness lock at {}",
            path.display()
        ),
    )
}
fn recover_stale_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    recover_stale_example_harness_lock_while_locked(path)
}
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
fn remove_stale_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    remove_stale_example_harness_lock_while_locked(path)
}
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

fn example_harness_liveness_lock_is_contended(error: &io::Error) -> bool {
    error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
}

/// Resolve the Cargo output directory used by this test process.
///
/// Cargo's `.rustc_info.json` marker is authoritative when present, because it
/// records the directory the toolchain actually wrote to. Without it, the
/// executable's profile directory is used, so marker-free processes still
/// agree on one lock identity. An absolute `CARGO_TARGET_DIR` is last, and a
/// relative one is ignored: Cargo resolves such a value from its invocation
/// directory, which this process cannot observe.
///
/// # Errors
///
/// Returns an error when filesystem metadata cannot be read or no target path
/// resolves.
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
    executable
        .ancestors()
        .find(|ancestor| ancestor.ends_with("deps"))
        .and_then(Path::parent)
}

fn write_lock_owner(path: &Path, owner: &ExampleHarnessLockOwner) -> io::Result<()> {
    std::fs::write(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME), &owner.0)
}

fn read_example_harness_lock_owner(path: &Path) -> io::Result<Option<ExampleHarnessLockOwner>> {
    match std::fs::read_to_string(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME)) {
        Ok(owner) => Ok(Some(ExampleHarnessLockOwner(owner))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

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

fn example_harness_lock_is_stale(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .is_ok_and(|age| age > EXAMPLE_HARNESS_LOCK_STALE_AFTER)
}

#[cfg(test)]
#[path = "harness_lock_tests.rs"]
mod tests;
