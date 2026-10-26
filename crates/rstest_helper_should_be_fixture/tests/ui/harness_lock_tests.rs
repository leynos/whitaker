//! Adapter and property tests for the example-harness lock protocol.
//!
//! Generated target paths exercise stable lock mapping, while focused cases
//! cover collision serialization and stale-owner recovery.

use super::super::unique_summary_path;
use super::*;
use filetime::{FileTime, set_file_mtime};
use proptest::prelude::*;
use rstest::rstest;

/// Set a lock directory timestamp beyond the stale-recovery threshold.
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

proptest! {
    /// Check deterministic mapping for generated target-directory paths.
    #[test]
    fn lock_path_mapping_is_deterministic_for_generated_target_directories(
        components in prop::collection::vec("[a-z0-9_-]{1,20}", 1..8),
    ) {
        let target_deps_directory = components
            .iter()
            .fold(PathBuf::from("worktrees"), |path, part| path.join(part))
            .join("target")
            .join("debug")
            .join("deps");
        let lock_path = example_harness_lock_path(&target_deps_directory);
        let repeated_lock_path = example_harness_lock_path(&target_deps_directory);

        prop_assert_eq!(&lock_path, &repeated_lock_path);
        prop_assert!(lock_path.starts_with(std::env::temp_dir()));
        prop_assert_eq!(lock_path.extension().and_then(|value| value.to_str()), Some("lock"));
    }

    /// Check distinct generated worktrees normally receive distinct lock paths.
    #[test]
    fn distinct_generated_worktrees_get_distinct_lock_paths(
        first_id in "[a-z0-9]{1,24}",
        second_id in "[a-z0-9]{1,24}",
    ) {
        prop_assume!(first_id != second_id);
        let first_target = PathBuf::from("worktrees")
            .join(first_id)
            .join("target")
            .join("debug")
            .join("deps");
        let second_target = PathBuf::from("worktrees")
            .join(second_id)
            .join("target")
            .join("debug")
            .join("deps");

        prop_assert_ne!(
            example_harness_lock_path(&first_target),
            example_harness_lock_path(&second_target),
        );
    }
}

/// Check the production acquisition path scopes its lock to the test binary.
#[test]
fn production_acquisition_uses_current_executable_target_directory() {
    let executable = std::env::current_exe().expect("resolve the current test executable");
    let target_deps_directory = executable
        .parent()
        .expect("the current test executable should have a parent directory");
    let expected_path = example_harness_lock_path(target_deps_directory);

    let lock = ExampleHarnessLock::acquire().expect("acquire the production harness lock");

    assert_eq!(lock.path(), expected_path.as_path());
}

/// Check equal target-directory digests safely share one serialized lock.
#[test]
fn colliding_target_digests_share_a_serialized_lock() {
    let unique_target = unique_summary_path();
    let mut hasher = DefaultHasher::new();
    unique_target.hash(&mut hasher);
    let digest = hasher.finish();
    let first_path = example_harness_lock_path_for_digest(digest);
    let second_path = example_harness_lock_path_for_digest(digest);

    assert_eq!(first_path, second_path);
    let first = ExampleHarnessLock::acquire_at(&first_path, None)
        .expect("first colliding target should acquire the shared lock");
    let Err(error) = ExampleHarnessLock::acquire_at(&second_path, Some(Duration::ZERO)) else {
        panic!("a colliding target must wait on the same lock");
    };
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);

    drop(first);
    let _second = ExampleHarnessLock::acquire_at(&second_path, Some(Duration::ZERO))
        .expect("the colliding target should acquire the released lock");
}

#[rstest]
#[case(Duration::from_mins(1), false)]
#[case(EXAMPLE_HARNESS_LOCK_STALE_AFTER + Duration::from_secs(1), true)]
/// Check the stale-lock threshold at representative ages.
fn example_harness_lock_stale_policy(#[case] age: Duration, #[case] expected: bool) {
    let now = SystemTime::now();
    let modified = now - age;

    assert_eq!(example_harness_lock_is_stale(modified, now), expected);
}

#[rstest]
#[case(true)]
#[case(false)]
/// Check recovery treats an absent lock directory as already released.
fn stale_lock_operations_treat_missing_directory_as_released(#[case] recover: bool) {
    let path = unique_summary_path();
    let operation = if recover {
        recover_stale_example_harness_lock(&path)
    } else {
        remove_stale_example_harness_lock(&path)
    };

    operation.expect("missing lock directory should be released");
}

#[test]
/// Check active contention returns a timeout at the configured boundary.
fn example_harness_lock_reports_active_contention_timeout() {
    let path = unique_summary_path();
    std::fs::create_dir(&path).expect("create test lock directory");
    let Err(error) = ExampleHarnessLock::acquire_at(&path, Some(Duration::ZERO)) else {
        panic!("active lock contention should time out");
    };
    let _cleanup_result = std::fs::remove_dir(&path);
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
/// Check another live owner is reported as lock contention.
fn held_example_harness_liveness_lock_is_reported_as_contended() {
    let path = unique_summary_path();
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
/// Check stale recovery preserves a live owner and reclaims after release.
fn stale_recovery_preserves_active_owner_then_reclaims_after_release() {
    let path = unique_summary_path();
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

/// Adapter coverage for the model's owner-token cleanup boundary.
#[test]
/// Check cleanup preserves a lock owned by a successor.
fn lock_cleanup_does_not_remove_a_different_owner() {
    let path = unique_summary_path();
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
