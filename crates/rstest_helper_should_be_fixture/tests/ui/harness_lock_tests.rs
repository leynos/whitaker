//! Unit tests for target-scoped example-harness locking and stale recovery.

use filetime::{FileTime, set_file_mtime};
use proptest::prelude::*;
use rstest::rstest;

use super::*;

/// Check the stale-age boundary for representative lock-directory ages.
#[rstest]
#[case(Duration::from_mins(1), false)]
#[case(EXAMPLE_HARNESS_LOCK_STALE_AFTER + Duration::from_secs(1), true)]
fn example_harness_lock_stale_policy(#[case] age: Duration, #[case] expected: bool) {
    let now = SystemTime::now();
    let modified = now - age;

    assert_eq!(example_harness_lock_is_stale(modified, now), expected);
}

/// Treat missing lock paths as released for each recovery operation.
#[rstest]
#[case(true)]
#[case(false)]
fn stale_lock_operations_treat_missing_directory_as_released(#[case] recover: bool) {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let path = test_directory.path().join("harness.lock");
    let operation = if recover {
        recover_stale_example_harness_lock(&path)
    } else {
        remove_stale_example_harness_lock(&path)
    };

    operation.expect("missing lock directory should be released");
}

/// Report a timeout when another owner holds the requested lock directory.
#[test]
fn example_harness_lock_reports_active_contention_timeout() {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let path = test_directory.path().join("harness.lock");
    std::fs::create_dir(&path).expect("create test lock directory");
    let Err(error) = ExampleHarnessLock::acquire_at(&path, Some(Duration::ZERO)) else {
        panic!("active lock contention should time out");
    };

    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

/// Resolve the target directory that contains the running Cargo test binary.
#[test]
fn effective_target_directory_contains_the_running_test_binary() {
    let target_directory = effective_target_directory().expect("resolve Cargo target directory");
    let executable = std::env::current_exe().expect("resolve current test executable");

    assert!(
        executable.starts_with(target_directory),
        "target directory should contain the test executable"
    );
}

/// Infer the active profile directory from a Cargo test binary path.
#[rstest]
#[case("debug")]
#[case("release")]
#[case("custom-profile")]
fn target_profile_directory_follows_test_binary_path(#[case] profile: &str) {
    let target_directory = tempfile::tempdir().expect("create target directory");
    let profile_directory = target_directory.path().join(profile);
    let executable = profile_directory.join("deps").join("test-binary.exe");

    assert_eq!(
        target_profile_directory_for_test_binary(&executable),
        Some(profile_directory.as_path())
    );
}

/// Exercise target-scoped acquisition, shared-target contention, and separation.
#[test]
fn acquisition_uses_the_injected_target_directory() {
    let shared_target = tempfile::tempdir().expect("create shared target directory");
    let independent_target = tempfile::tempdir().expect("create independent target directory");
    let lock_root = tempfile::tempdir().expect("create temporary lock root");
    let shared_path = example_harness_lock_path(shared_target.path(), lock_root.path());
    let independent_path = example_harness_lock_path(independent_target.path(), lock_root.path());

    assert_ne!(
        shared_path, independent_path,
        "independent target directories should have distinct locks"
    );

    let shared_lock = ExampleHarnessLock::acquire_for_target_directory(
        shared_target.path(),
        lock_root.path(),
        Some(Duration::ZERO),
    )
    .expect("first shared-target acquisition should succeed");
    assert_eq!(shared_lock.path(), shared_path);

    let Err(error) = ExampleHarnessLock::acquire_for_target_directory(
        shared_target.path(),
        lock_root.path(),
        Some(Duration::ZERO),
    ) else {
        panic!("a second shared-target acquisition should wait");
    };
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);

    let independent_lock = ExampleHarnessLock::acquire_for_target_directory(
        independent_target.path(),
        lock_root.path(),
        Some(Duration::ZERO),
    )
    .expect("independent target acquisition should succeed");
    assert_eq!(independent_lock.path(), independent_path);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        .. ProptestConfig::default()
    })]

    #[doc = "Check stable mapping and safe collision serialization across varied paths."]
    #[test]
    fn target_lock_paths_follow_the_hash_collision_policy(
        first_components in prop::collection::vec("[a-zA-Z0-9_-]{1,16}", 1..6),
        second_components in prop::collection::vec("[a-zA-Z0-9_-]{1,16}", 1..6),
    ) {
        let lock_root = PathBuf::from("temporary-lock-root");
        let first_target = first_components
            .into_iter()
            .fold(PathBuf::from("target"), |path, component| path.join(component));
        let second_target = second_components
            .into_iter()
            .fold(PathBuf::from("target"), |path, component| path.join(component));

        let first_lock = example_harness_lock_path(&first_target, &lock_root);
        let second_lock = example_harness_lock_path(&second_target, &lock_root);
        prop_assert_eq!(
            &first_lock,
            &example_harness_lock_path(&first_target, &lock_root),
            "one target directory should always map to the same lock"
        );
        prop_assert_eq!(
            first_lock == second_lock,
            example_harness_lock_key(&first_target) == example_harness_lock_key(&second_target),
            "only equal hash keys may share a lock; collisions serialize safely"
        );
    }
}

/// Preserve deterministic mapping for absolute, relative, parent, and spaced paths.
#[test]
fn target_lock_paths_are_stable_for_path_edge_forms() {
    let lock_root = tempfile::tempdir().expect("create temporary lock root");
    let target_paths = [
        PathBuf::from("target"),
        PathBuf::from("../target"),
        PathBuf::from("target with spaces"),
        std::env::temp_dir().join("absolute target"),
    ];

    for target_path in target_paths {
        assert_eq!(
            example_harness_lock_path(&target_path, lock_root.path()),
            example_harness_lock_path(&target_path, lock_root.path()),
            "path form should map deterministically: {}",
            target_path.display()
        );
    }
}

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
