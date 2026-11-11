//! Recovery tests for the example-harness lock protocol.

use filetime::{FileTime, set_file_mtime};

use super::super::*;

/// Report contention while another process holds the liveness sidecar.
#[test]
fn held_example_harness_liveness_lock_is_reported_as_contended() {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let path = test_directory.path().join("harness.lock");
    let first = try_lock_example_harness_liveness(&path)
        .expect("acquire liveness lock")
        .expect("first liveness lock should be available");
    assert!(
        try_lock_example_harness_liveness(&path)
            .expect("check liveness lock contention")
            .is_none()
    );
    drop(first);
}

/// Reclaim stale state only after the active owner releases its liveness lock.
#[test]
fn stale_recovery_preserves_active_owner_then_reclaims_after_release() {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let path = test_directory.path().join("harness.lock");
    let owner = ExampleHarnessLock::acquire_at(&path, None).expect("acquire active owner");
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

/// Reclaim a released lock without waiting out the stale grace period.
///
/// A crashed process drops its liveness lock immediately, so an abandoned
/// directory can be reused as soon as the OS reports the release. Only the
/// owner-aware removal path decides eligibility; the age merely classifies the
/// event for diagnostics.
#[test]
fn recovery_reclaims_a_recently_abandoned_lock() {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let path = test_directory.path().join("harness.lock");
    std::fs::create_dir(&path).expect("create abandoned lock directory");
    write_lock_owner(&path, &ExampleHarnessLockOwner::new()).expect("write abandoned owner");

    recover_stale_example_harness_lock(&path).expect("reclaim abandoned lock");

    assert!(
        !path.exists(),
        "an abandoned lock should be reclaimed without waiting for its age"
    );
}

/// Preserve a successor-owned lock when the previous owner is dropped.
#[test]
fn lock_cleanup_does_not_remove_a_different_owner() {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let path = test_directory.path().join("harness.lock");
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
    remove_stale_example_harness_lock(&path).expect("remove released different-owner directory");
}

/// Age a lock directory for stale-recovery tests.
fn make_example_harness_lock_stale(path: &Path) {
    let Some(stale_modified) =
        SystemTime::now().checked_sub(EXAMPLE_HARNESS_LOCK_STALE_AFTER + Duration::from_secs(1))
    else {
        panic!("stale timestamp should be representable")
    };
    if let Err(error) = set_file_mtime(path, FileTime::from_system_time(stale_modified)) {
        panic!("lock mtime should be adjustable: {error}");
    }
}
