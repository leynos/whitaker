//! Example-harness lock protocol for the UI test binary.
//!
//! `tests/ui.rs`'s `ExampleHarness` fixture serializes example compilations
//! across separate nextest processes with a filesystem lock. This module owns
//! that protocol — the lock type, its stale-owner recovery, and the adapter
//! tests that exercise it — so `ui.rs` stays focused on the example and
//! trybuild assertions.

use filetime::{FileTime, set_file_mtime};
use fs2::FileExt;
use log::debug;
use rstest::rstest;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// The example harness lock coordinates separate nextest processes. Windows CI
// can legitimately hold it for several minutes, so only remove directories
// old enough to be abandoned by a crashed process.
const EXAMPLE_HARNESS_LOCK_STALE_AFTER: Duration = Duration::from_secs(30 * 60);
// Bound the default `acquire()` wait so a wedged live owner surfaces a timeout
// instead of polling forever. It exceeds the stale-recovery window so genuinely
// abandoned locks are reclaimed before this ceiling is reached.
const EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(31 * 60);
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
            std::env::temp_dir().join("rstest-helper-example-harness.lock"),
            Some(EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT),
        )
    }

    fn acquire_at(path: PathBuf, wait_limit: Option<Duration>) -> io::Result<Self> {
        let started_at = Instant::now();
        let mut attempt = 0_u64;
        loop {
            attempt += 1;
            let state_guard = lock_example_harness_state(&path)?;
            match create_example_harness_lock(path.clone()) {
                Ok(lock) => {
                    debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=acquired path={} owner={} attempt={} elapsed_ms={}", path.display(), lock.owner.0, attempt, started_at.elapsed().as_millis());
                    return Ok(lock);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    debug!(target: EXAMPLE_HARNESS_LOCK_LOG_TARGET, "event=contended path={} attempt={} elapsed_ms={}", path.display(), attempt, started_at.elapsed().as_millis());
                    recover_stale_example_harness_lock_while_locked(&path)?;
                }
                Err(error) => return Err(error),
            }
            drop(state_guard);
            wait_for_example_harness_lock_release(&path, started_at, wait_limit)?;
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
        let _ = remove_example_harness_lock_directory(&path);
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
            let _ = remove_lock_if_owned(&self.path, &self.owner);
        }
        // Release liveness after owner-aware cleanup to avoid successor races.
        let _ = FileExt::unlock(&self.owner_liveness);
    }
}

fn wait_for_example_harness_lock_release(
    path: &Path,
    started_at: Instant,
    wait_limit: Option<Duration>,
) -> io::Result<()> {
    recover_stale_example_harness_lock(path)?;
    if wait_limit.is_some_and(|wait_limit| started_at.elapsed() >= wait_limit) {
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
    match read_example_harness_lock_owner(path)? {
        Some(owner) => remove_lock_if_owned(path, &owner),
        None => remove_example_harness_lock_directory(path),
    }
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

fn example_harness_lock_is_stale(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .is_ok_and(|age| age > EXAMPLE_HARNESS_LOCK_STALE_AFTER)
}

fn make_example_harness_lock_stale(path: &Path) {
    let stale_modified = SystemTime::now()
        .checked_sub(EXAMPLE_HARNESS_LOCK_STALE_AFTER + Duration::from_secs(1))
        .expect("stale timestamp should be representable");
    set_file_mtime(path, FileTime::from_system_time(stale_modified)).expect("adjust lock mtime");
}

#[rstest]
#[case(Duration::from_secs(60), false)]
#[case(EXAMPLE_HARNESS_LOCK_STALE_AFTER + Duration::from_secs(1), true)]
fn example_harness_lock_stale_policy(#[case] age: Duration, #[case] expected: bool) {
    let now = SystemTime::now();
    let modified = now - age;

    assert_eq!(example_harness_lock_is_stale(modified, now), expected);
}

#[rstest]
#[case(true)]
#[case(false)]
fn stale_lock_operations_treat_missing_directory_as_released(#[case] recover: bool) {
    let path = super::unique_summary_path();
    let operation = if recover {
        recover_stale_example_harness_lock(&path)
    } else {
        remove_stale_example_harness_lock(&path)
    };

    operation.expect("missing lock directory should be released");
}

#[test]
fn example_harness_lock_reports_active_contention_timeout() {
    let path = super::unique_summary_path();
    std::fs::create_dir(&path).expect("create test lock directory");
    let Err(error) = ExampleHarnessLock::acquire_at(path.clone(), Some(Duration::ZERO)) else {
        panic!("active lock contention should time out");
    };
    let _ = std::fs::remove_dir(&path);
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

/// Adapter coverage for the model's live-owner, release, then reclaim boundary.
#[test]
fn stale_recovery_preserves_active_owner_then_reclaims_after_release() {
    let path = super::unique_summary_path();
    let owner = ExampleHarnessLock::acquire_at(path.clone(), None).expect("acquire active owner");
    make_example_harness_lock_stale(&path);
    recover_stale_example_harness_lock(&path).expect("active owner blocks recovery");
    assert!(path.is_dir(), "live owner directory must remain intact");

    {
        let _state_guard = lock_example_harness_state(&path).expect("lock state");
        std::fs::remove_file(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME))
            .expect("remove owner metadata");
    }
    make_example_harness_lock_stale(&path);
    drop(owner);

    assert!(path.is_dir(), "stale directory remains");
    recover_stale_example_harness_lock(&path).expect("reclaim released stale owner");
    assert!(!path.exists(), "released stale directory should be removed");
}

/// Adapter coverage for the model's owner-token cleanup boundary.
#[test]
fn lock_cleanup_does_not_remove_a_different_owner() {
    let path = super::unique_summary_path();
    let original = ExampleHarnessLock::acquire_at(path.clone(), None).expect("acquire lock");
    let successor = ExampleHarnessLockOwner::new();
    {
        let _state_guard = lock_example_harness_state(&path).expect("lock state");
        write_lock_owner(&path, &successor).expect("replace owner metadata");
        remove_lock_if_owned(&path, &original.owner).expect("inspect successor ownership");
    }

    assert!(path.is_dir());
    drop(original);
    remove_stale_example_harness_lock(&path).expect("remove released different-owner directory");
}
