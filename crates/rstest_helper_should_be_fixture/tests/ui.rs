//! UI harness for the `rstest_helper_should_be_fixture` lint.
//!
//! These fixtures execute the lint driver and keep the current
//! diagnostic-silent contract. The example harness asserts the driver-owned
//! collector records real call-site evidence, while the trybuild cases retain
//! compile-time coverage for the same source shapes without depending on
//! diagnostics that later roadmap tasks will introduce.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use dylint_testing::ui::Test;
use fs2::FileExt;
use rstest::rstest;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use whitaker_common::test_support::{EnvVarGuard, run_test_runner};

// Internal test-only hook mirrored in the lint driver. It asks
// `check_crate_post` to append redacted, shape-only passive collection
// summaries for harness assertions without making the lint user-visible.
const COLLECTION_SUMMARY_ENV: &str = "WHITAKER_RSTEST_HELPER_COLLECTION_SUMMARY";
// The example harness lock coordinates separate nextest processes. Windows CI
// can legitimately hold it for several minutes, so only remove directories
// old enough to be abandoned by a crashed process.
const EXAMPLE_HARNESS_LOCK_STALE_AFTER: Duration = Duration::from_secs(30 * 60);
const EXAMPLE_HARNESS_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(100);
const EXAMPLE_HARNESS_LOCK_OWNER_FILENAME: &str = "owner";

#[rstest]
#[case("bootstrap_zero_diagnostic")]
#[case("collection_zero_diagnostic")]
fn example_compiles_without_diagnostics(#[case] example: &str) {
    run_example(example);
}

#[test]
fn example_harness_collects_call_site_evidence() {
    let summary_path = unique_summary_path();
    let _guard = EnvVarGuard::set(COLLECTION_SUMMARY_ENV, summary_path.as_os_str());

    run_example("collection_zero_diagnostic");

    let summary =
        std::fs::read_to_string(&summary_path).expect("collection summary should be written");
    let _ = std::fs::remove_file(&summary_path);

    assert!(summary.contains("callee_count=3"), "{summary}");
    assert!(summary.contains("record_count=3"), "{summary}");
    assert!(
        summary.contains("callee=Builder::<'_>::build;records=1"),
        "{summary}"
    );
    assert!(summary.contains("callee=helper;records=1"), "{summary}");
    assert!(
        summary.contains("callee=nested_helper;records=1"),
        "{summary}"
    );
    assert!(
        summary.contains("fingerprint=unsupported,fixture-local"),
        "{summary}"
    );
    assert!(
        summary.contains("fingerprint=fixture-local,fixture-local,const-path,const-path"),
        "{summary}"
    );
    assert!(!summary.contains("literal"), "{summary}");
}

#[test]
fn collection_summary_paths_are_fresh_per_call() {
    assert_ne!(unique_summary_path(), unique_summary_path());
}

#[test]
fn trybuild_fixtures_compile_without_diagnostics() {
    let cases = trybuild::TestCases::new();

    cases.pass("tests/ui/bootstrap_zero_diagnostic.rs");
    cases.pass("tests/ui/collection_zero_diagnostic.rs");
}

fn run_example(example: &str) {
    let _lock = ExampleHarnessLock::acquire().expect("example harness lock should be acquired");
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner(example, || {
            let mut test = Test::example(crate_name, example);
            test.rustc_flags(["--test"]);
            test.run();
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "UI tests should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error} }}"
        )
    });
}

struct ExampleHarnessLock {
    path: PathBuf,
    owner: ExampleHarnessLockOwner,
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
    fn acquire() -> io::Result<Self> {
        let path = std::env::temp_dir().join("rstest-helper-example-harness.lock");
        Self::acquire_at(path, None)
    }

    fn acquire_at(path: PathBuf, wait_limit: Option<Duration>) -> io::Result<Self> {
        let started_at = Instant::now();

        loop {
            let state_guard = lock_example_harness_state(&path)?;
            match create_example_harness_lock(path.clone()) {
                Ok(lock) => return Ok(lock),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
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
    std::fs::create_dir(&path)?;
    let owner = ExampleHarnessLockOwner::new();
    if let Err(error) = write_example_harness_lock_owner(&path, &owner) {
        let _ = remove_example_harness_lock_directory(&path);
        return Err(error);
    }

    Ok(ExampleHarnessLock { path, owner })
}

impl Drop for ExampleHarnessLock {
    fn drop(&mut self) {
        if let Ok(_state_guard) = lock_example_harness_state(&self.path) {
            let _ = remove_example_harness_lock_if_owned(&self.path, &self.owner);
        }
    }
}

fn wait_for_example_harness_lock_release(
    path: &Path,
    started_at: Instant,
    wait_limit: Option<Duration>,
) -> io::Result<()> {
    recover_stale_example_harness_lock(path)?;
    if wait_limit.is_some_and(|wait_limit| started_at.elapsed() >= wait_limit) {
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
            path.display(),
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
        remove_stale_example_harness_lock_while_locked(path)?;
    }

    Ok(())
}

fn remove_stale_example_harness_lock(path: &Path) -> io::Result<()> {
    let _state_guard = lock_example_harness_state(path)?;
    remove_stale_example_harness_lock_while_locked(path)
}

fn remove_stale_example_harness_lock_while_locked(path: &Path) -> io::Result<()> {
    match read_example_harness_lock_owner(path)? {
        Some(owner) => remove_example_harness_lock_if_owned(path, &owner),
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

fn write_example_harness_lock_owner(
    path: &Path,
    owner: &ExampleHarnessLockOwner,
) -> io::Result<()> {
    std::fs::write(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME), &owner.0)
}

fn read_example_harness_lock_owner(path: &Path) -> io::Result<Option<ExampleHarnessLockOwner>> {
    match std::fs::read_to_string(path.join(EXAMPLE_HARNESS_LOCK_OWNER_FILENAME)) {
        Ok(owner) => Ok(Some(ExampleHarnessLockOwner(owner))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn remove_example_harness_lock_if_owned(
    path: &Path,
    owner: &ExampleHarnessLockOwner,
) -> io::Result<()> {
    let Some(current_owner) = read_example_harness_lock_owner(path)? else {
        return Ok(());
    };
    if current_owner != *owner {
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

fn unique_summary_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);

    std::env::temp_dir().join(format!(
        "rstest-helper-collection-{}-{}.txt",
        std::process::id(),
        suffix,
    ))
}

#[rstest]
#[case(Duration::from_secs(60), false)]
#[case(EXAMPLE_HARNESS_LOCK_STALE_AFTER + Duration::from_secs(1), true)]
fn example_harness_lock_stale_policy(#[case] age: Duration, #[case] expected: bool) {
    let now = SystemTime::now();
    let modified = now - age;

    assert_eq!(example_harness_lock_is_stale(modified, now), expected);
}

#[test]
fn stale_lock_recovery_treats_missing_directory_as_released() {
    let path = unique_summary_path();

    recover_stale_example_harness_lock(&path)
        .expect("missing lock directory should be treated as already released");
}

#[test]
fn example_harness_lock_reports_active_contention_timeout() {
    let path = unique_summary_path();
    std::fs::create_dir(&path).expect("test lock directory should be created");

    let Err(error) = ExampleHarnessLock::acquire_at(path.clone(), Some(Duration::ZERO)) else {
        panic!("active lock contention should time out");
    };
    let _ = std::fs::remove_dir(&path);

    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn stale_lock_removal_treats_missing_directory_as_released() {
    let path = unique_summary_path();

    remove_stale_example_harness_lock(&path)
        .expect("missing stale lock directory should be treated as already released");
}

#[test]
fn original_lock_owner_does_not_remove_a_successor() {
    let path = unique_summary_path();
    let original = ExampleHarnessLock::acquire_at(path.clone(), None)
        .expect("original lock should be acquired");
    {
        let _state_guard = lock_example_harness_state(&path)
            .expect("lock state should serialize the simulated takeover");
        remove_example_harness_lock_if_owned(&path, &original.owner)
            .expect("simulated stale takeover should remove the original lock");
    }

    let successor = ExampleHarnessLock::acquire_at(path.clone(), None)
        .expect("successor lock should be acquired");
    drop(original);

    assert!(path.is_dir());
    drop(successor);
}
