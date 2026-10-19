//! Example-harness lock protocol for the UI test binary.
//!
//! `tests/ui.rs`'s `ExampleHarness` fixture serializes example compilations
//! across separate nextest processes with a filesystem lock. This module owns
//! that protocol — the lock type, abandoned-owner recovery, and the adapter
//! tests that exercise it — so `ui.rs` stays focused on the example and
//! trybuild assertions.

use std::{
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use fs2::FileExt;
use log::debug;
use rstest::rstest;

// Bound the wait so a wedged live owner surfaces a timeout instead of polling
// forever. Abandoned locks are reclaimed as soon as their liveness lock drops.
const EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_mins(31);
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
    /// Returns the lock directory serializing the current example run.
    pub(crate) fn path(&self) -> &Path { &self.path }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ExampleHarnessLockOwner(String);
impl ExampleHarnessLockOwner {
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
    pub(crate) fn acquire() -> io::Result<Self> {
        Self::acquire_at(
            &std::env::temp_dir().join("rstest-helper-example-harness.lock"),
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
                    recover_abandoned_example_harness_lock_while_locked(path)?;
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
    recover_abandoned_example_harness_lock(path)?;
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
fn recover_abandoned_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    recover_abandoned_example_harness_lock_while_locked(path)
}
fn recover_abandoned_example_harness_lock_while_locked(path: &Path) -> io::Result<()> {
    match std::fs::metadata(path) {
        Ok(_) => remove_released_example_harness_lock_while_locked(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
fn remove_released_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    remove_released_example_harness_lock_while_locked(path)
}
fn remove_released_example_harness_lock_while_locked(path: &Path) -> io::Result<()> {
    let Some(_owner_liveness) = try_lock_example_harness_liveness(path)? else {
        debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=recovery_live_owner path={}", path.display());
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

#[rstest]
#[case(true)]
#[case(false)]
fn released_lock_operations_treat_missing_directory_as_released(#[case] recover: bool) {
    let path = super::unique_summary_path();
    let operation = if recover {
        recover_abandoned_example_harness_lock(&path)
    } else {
        remove_released_example_harness_lock(&path)
    };

    operation.expect("missing lock directory should be released");
}

#[test]
fn example_harness_lock_reports_active_contention_timeout() {
    let path = super::unique_summary_path();
    let _owner_liveness = try_lock_example_harness_liveness(&path)
        .expect("open liveness lock")
        .expect("acquire active liveness lock");
    std::fs::create_dir(&path).expect("create test lock directory");
    let Err(error) = ExampleHarnessLock::acquire_at(&path, Some(Duration::ZERO)) else {
        panic!("active lock contention should time out");
    };
    let _cleanup_result = std::fs::remove_dir(&path);
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn held_example_harness_liveness_lock_is_reported_as_contended() {
    let path = super::unique_summary_path();
    let first = try_lock_example_harness_liveness(&path)
        .expect("acquire liveness lock")
        .expect("first liveness lock should be available");
    assert!(
        try_lock_example_harness_liveness(&path)
            .expect("check liveness lock contention")
            .is_none()
    );
    drop(first);
    let liveness_path = path.with_extension(EXAMPLE_HARNESS_LOCK_LIVENESS_EXTENSION);
    if let Err(error) = std::fs::remove_file(liveness_path) {
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}

/// A live owner is preserved until its liveness lock is released.
#[test]
fn recovery_preserves_a_live_owner() {
    let path = super::unique_summary_path();
    let owner = ExampleHarnessLock::acquire_at(&path, None).expect("acquire active owner");
    recover_abandoned_example_harness_lock(&path).expect("active owner blocks recovery");
    assert!(path.is_dir(), "live owner directory must remain intact");
    drop(owner);
}

/// A recent directory without a live owner is reclaimed immediately.
#[test]
fn recovery_reclaims_a_recent_abandoned_lock() {
    let path = super::unique_summary_path();
    std::fs::create_dir(&path).expect("create abandoned lock directory");
    write_lock_owner(&path, &ExampleHarnessLockOwner::new()).expect("write abandoned owner");

    recover_abandoned_example_harness_lock(&path).expect("reclaim abandoned lock");
    assert!(!path.exists(), "abandoned lock should be removed");
}

/// Adapter coverage for the model's owner-token cleanup boundary.
#[test]
fn lock_cleanup_does_not_remove_a_different_owner() {
    let path = super::unique_summary_path();
    let original = ExampleHarnessLock::acquire_at(&path, None).expect("acquire lock");
    let successor = ExampleHarnessLockOwner::new();
    {
        let _state_guard = lock_example_harness_state(&path).expect("lock state");
        write_lock_owner(&path, &successor).expect("replace owner metadata");
        remove_lock_if_owned(&path, &original.owner).expect("inspect successor ownership");
    }

    assert!(
        path.is_dir(),
        "a successor-owned lock directory must survive"
    );
    drop(original);
    remove_released_example_harness_lock(&path).expect("remove released different-owner directory");
}
