//! Process-level tests for target-scoped example-harness locking.

use std::{io, path::Path, time::Duration};

use super::super::{
    EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT, ExampleHarnessLock, effective_target_directory,
};

const CHILD_LOCK_MODE_ENV: &str = "WHITAKER_EXAMPLE_HARNESS_LOCK_CHILD_MODE";

/// Share target-lock contention and release across separate test processes.
#[test]
fn cross_process_lock_contention_and_release() {
    match std::env::var(CHILD_LOCK_MODE_ENV) {
        Ok(mode) => exercise_child_lock_mode(&mode),
        Err(std::env::VarError::NotPresent) => {
            exercise_parent_child_lock_contention_and_release();
        }
        Err(error) => panic!("child lock mode should be valid UTF-8: {error}"),
    }
}

/// Assert the child lock attempt matches the operation selected by the parent.
fn exercise_child_lock_mode(mode: &str) {
    let executable = std::env::current_exe().expect("resolve child test executable");
    let configured_target_directory = std::env::var_os("CARGO_TARGET_DIR");
    let target_directory = effective_target_directory(
        &executable,
        configured_target_directory.as_deref().map(Path::new),
    )
    .expect("resolve child Cargo target directory");
    let lock_result = ExampleHarnessLock::acquire_for_target_directory(
        &target_directory,
        &target_directory,
        Some(Duration::ZERO),
    );

    match mode {
        "contended" => match lock_result {
            Err(error) => assert_eq!(error.kind(), io::ErrorKind::TimedOut),
            Ok(lock) => panic!(
                "child acquired a lock held by its parent: {}",
                lock.path().display()
            ),
        },
        "acquire" => {
            let lock = lock_result.expect("released parent lock should be acquirable");
            drop(lock);
        }
        other => panic!("unknown child lock mode: {other}"),
    }
}

/// Hold the target lock while a child process contends, then verify release.
fn exercise_parent_child_lock_contention_and_release() {
    let executable = std::env::current_exe().expect("resolve parent test executable");
    let configured_target_directory = std::env::var_os("CARGO_TARGET_DIR");
    let target_directory = effective_target_directory(
        &executable,
        configured_target_directory.as_deref().map(Path::new),
    )
    .expect("resolve parent Cargo target directory");
    let parent_lock = ExampleHarnessLock::acquire_for_target_directory(
        &target_directory,
        &target_directory,
        Some(EXAMPLE_HARNESS_LOCK_ACQUIRE_TIMEOUT),
    )
    .expect("parent should acquire the effective-target lock");

    let contended_child = run_child_lock_test(&target_directory, "contended");
    assert_successful_lock_child(contended_child, "contended");
    drop(parent_lock);

    let acquiring_child = run_child_lock_test(&target_directory, "acquire");
    assert_successful_lock_child(acquiring_child, "acquire");
}

/// Run only the lock test in a fresh copy of this test binary.
fn run_child_lock_test(target_directory: &Path, mode: &str) -> std::process::Output {
    let executable = std::env::current_exe().expect("resolve current test executable");
    std::process::Command::new(executable)
        .arg("--nocapture")
        .arg("cross_process_lock_contention_and_release")
        .env(CHILD_LOCK_MODE_ENV, mode)
        .env("CARGO_TARGET_DIR", target_directory)
        .output()
        .expect("run child lock test")
}

/// Verify a child ran the requested test case and exited successfully.
fn assert_successful_lock_child(output: std::process::Output, mode: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{mode} child should pass; stdout: {stdout}; stderr: {stderr}"
    );
    assert!(
        stdout.contains("running 1 test")
            && stdout.contains("cross_process_lock_contention_and_release"),
        "{mode} child should run exactly the lock test; stdout: {stdout}; stderr: {stderr}"
    );
}

/// Resolve an absolute target override when the executable has no Cargo marker.
#[test]
fn effective_target_directory_uses_absolute_override() {
    let test_directory = tempfile::tempdir().expect("create test directory");
    let executable_root = test_directory.path().join("executable-root");
    let target_directory = test_directory.path().join("configured-target");
    let executable = executable_root.join("debug/deps/test-binary");
    std::fs::create_dir_all(executable.parent().expect("get executable directory"))
        .expect("create executable directory");
    std::fs::create_dir(&target_directory).expect("create configured target directory");
    let expected_target_directory =
        std::fs::canonicalize(&target_directory).expect("canonicalize configured target");

    assert_eq!(
        effective_target_directory(&executable, Some(target_directory.as_path()))
            .expect("resolve absolute target override"),
        expected_target_directory
    );
}
