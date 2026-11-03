//! Adapter tests for the target-scoped example-harness lock.
//!
//! Compiled only inside `harness_lock`'s test module. Each case uses its
//! own temporary directory so the suite never contends with the real
//! target-directory lock it is validating.

use filetime::{FileTime, set_file_mtime};
use rstest::rstest;

use super::*;

/// Age a lock directory past the stale-recovery threshold.
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

/// Create an isolated lock path inside a temporary parent directory.
///
/// The directory is returned so it outlives every lock acquired from the path
/// and is removed however the test exits.
fn isolated_lock_path() -> (tempfile::TempDir, PathBuf) {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let path = test_directory.path().join("harness.lock");
    (test_directory, path)
}

/// The lock lives inside the resolved target directory.
///
/// Regression: a host-global `temp_dir()` path made every worktree on the host
/// serialize against one lock, so one worktree's slow build could block another
/// for the whole stale window.
#[test]
fn acquire_places_the_lock_inside_the_effective_target_directory() {
    let executable = std::env::current_exe().expect("resolve current test executable");
    let configured_target_directory = std::env::var_os("CARGO_TARGET_DIR");
    let target_directory = effective_target_directory(
        &executable,
        configured_target_directory.as_deref().map(Path::new),
    )
    .expect("resolve Cargo target directory");

    let lock = ExampleHarnessLock::acquire().expect("acquire the effective-target lock");

    assert_eq!(
        lock.path(),
        target_directory.join("rstest-helper-example-harness.lock")
    );
}

/// Resolve the target root marked by Cargo's underscore-named metadata file.
#[test]
fn effective_target_directory_uses_cargo_marker() {
    let target_directory = tempfile::tempdir().expect("create target directory");
    let executable = target_directory.path().join("debug/deps/test-binary");
    std::fs::create_dir_all(executable.parent().expect("get executable directory"))
        .expect("create executable directory");
    std::fs::write(target_directory.path().join(".rustc_info.json"), "{}")
        .expect("create Cargo target marker");

    assert_eq!(
        effective_target_directory(&executable, None).expect("resolve marked target directory"),
        std::fs::canonicalize(target_directory.path()).expect("canonicalize target directory")
    );
}

/// Prefer Cargo's marker over a conflicting absolute target override.
#[test]
fn effective_target_directory_prefers_cargo_marker_over_absolute_override() {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let marker_directory = test_directory.path().join("marked-target");
    let executable = marker_directory.join("debug/deps/test-binary");
    std::fs::create_dir_all(executable.parent().expect("get executable directory"))
        .expect("create executable directory");
    std::fs::write(marker_directory.join(".rustc_info.json"), "{}")
        .expect("create Cargo target marker");
    let override_directory = test_directory.path().join("override-target");
    std::fs::create_dir(&override_directory).expect("create override directory");
    let expected_directory =
        std::fs::canonicalize(&marker_directory).expect("canonicalize marked target");

    assert_eq!(
        effective_target_directory(&executable, Some(&override_directory))
            .expect("resolve the marker-bearing target directory"),
        expected_directory
    );
}

/// Return `NotFound` when neither Cargo metadata nor a target path resolves.
#[test]
fn effective_target_directory_reports_not_found_without_resolution_sources() {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let executable = test_directory.path().join("bin/test-binary");
    std::fs::create_dir_all(executable.parent().expect("get executable directory"))
        .expect("create executable directory");

    let error = effective_target_directory(&executable, None)
        .expect_err("unresolvable target directory should report NotFound");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

/// Ignore a relative target override resolved in another process's directory.
#[test]
fn effective_target_directory_ignores_relative_override() {
    let target_directory = tempfile::tempdir().expect("create target directory");
    let executable = target_directory.path().join("release/deps/test-binary");
    std::fs::create_dir_all(executable.parent().expect("get executable directory"))
        .expect("create executable directory");
    let profile_directory = std::fs::canonicalize(target_directory.path().join("release"))
        .expect("canonicalize profile directory");

    assert_eq!(
        effective_target_directory(&executable, Some(Path::new("relative-target")))
            .expect("resolve profile directory"),
        profile_directory
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

#[rstest]
#[case(Duration::from_mins(1), false)]
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
    let (_test_directory, path) = isolated_lock_path();
    let operation = if recover {
        recover_stale_example_harness_lock(&path)
    } else {
        remove_stale_example_harness_lock(&path)
    };

    operation.expect("missing lock directory should be released");
}

#[test]
fn example_harness_lock_reports_active_contention_timeout() {
    let (_test_directory, path) = isolated_lock_path();
    std::fs::create_dir(&path).expect("create test lock directory");
    let Err(error) = ExampleHarnessLock::acquire_at(&path, Some(Duration::ZERO)) else {
        panic!("active lock contention should time out");
    };
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn held_example_harness_liveness_lock_is_reported_as_contended() {
    let (_test_directory, path) = isolated_lock_path();
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

/// Adapter coverage for the model's live-owner, release, then reclaim boundary.
#[test]
fn stale_recovery_preserves_active_owner_then_reclaims_after_release() {
    let (_test_directory, path) = isolated_lock_path();
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
fn lock_cleanup_does_not_remove_a_different_owner() {
    let (_test_directory, path) = isolated_lock_path();
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
