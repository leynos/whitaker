//! Tests that verify the UI harness runner validates inputs and propagates
//! errors from custom runners.
use super::{HarnessError, run_with_runner};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use std::env;
#[cfg(windows)]
use std::sync::{Mutex, MutexGuard, OnceLock};
#[cfg(windows)]
use whitaker_common::test_support::EnvVarGuard;

#[rstest]
#[case(
    "  ",
    "ui",
    HarnessError::EmptyCrateName,
    "crate name validation should fail"
)]
#[case(
    "lint",
    "   ",
    HarnessError::EmptyDirectory,
    "empty directories should be rejected"
)]
fn rejects_invalid_inputs(
    #[case] crate_name: &str,
    #[case] directory: &str,
    #[case] expected: HarnessError,
    #[case] panic_message: &str,
) {
    let error = run_with_runner(crate_name, directory, |_, _| Ok(())).expect_err(panic_message);

    assert_eq!(error, expected);
}

#[test]
fn rejects_absolute_directories() {
    let current_dir = env::current_dir().expect("determine current directory");
    let absolute_directory = current_dir.join("ui");
    let path = Utf8PathBuf::from_path_buf(absolute_directory)
        .expect("workspace paths should be valid UTF-8");
    let error = run_with_runner("lint", path.clone(), |_, _| Ok(()))
        .expect_err("absolute directories should be rejected");

    assert_eq!(error, HarnessError::AbsoluteDirectory { directory: path });
}

#[cfg(windows)]
#[test]
fn rejects_unix_style_absolute_directories_on_windows() {
    let path = Utf8PathBuf::from("/tmp/ui");
    let error = run_with_runner("lint", path.clone(), |_, _| Ok(()))
        .expect_err("rooted paths should be rejected");

    assert_eq!(error, HarnessError::AbsoluteDirectory { directory: path });
}

#[cfg(windows)]
#[test]
fn rejects_unc_directories_on_windows() {
    let path = Utf8PathBuf::from(r"\\server\share\ui");
    let error = run_with_runner("lint", path.clone(), |_, _| Ok(()))
        .expect_err("UNC paths should be rejected");

    assert_eq!(error, HarnessError::AbsoluteDirectory { directory: path });
}

#[cfg(windows)]
#[test]
fn rejects_drive_relative_directories_on_windows() {
    let path = Utf8PathBuf::from("C:ui");
    let error = run_with_runner("lint", path.clone(), |_, _| Ok(()))
        .expect_err("drive-relative paths should be rejected");

    assert_eq!(error, HarnessError::AbsoluteDirectory { directory: path });
}

#[test]
fn propagates_runner_failures() {
    #[cfg(windows)]
    let _serial_guard = windows_env_guard_test_lock();

    let error = run_with_runner("lint", "ui", |crate_name, directory| {
        assert_eq!(crate_name, "lint");
        assert_eq!(directory, Utf8Path::new("ui"));
        Err(String::from("diff mismatch"))
    })
    .expect_err("runner failures should bubble up");

    assert_eq!(
        error,
        HarnessError::RunnerFailure {
            crate_name: String::from("lint"),
            directory: Utf8PathBuf::from("ui"),
            message: String::from("diff mismatch"),
        },
    );
}

#[cfg(windows)]
#[test]
fn windows_env_guard_clears_and_restores_rustc_wrapper() {
    let _serial_guard = windows_env_guard_test_lock();
    let _guard = EnvVarGuard::set("RUSTC_WRAPPER", "sccache");

    run_with_runner("lint", "ui", |_, _| {
        assert_eq!(env::var_os("RUSTC_WRAPPER"), None);
        Ok(())
    })
    .expect("runner should execute with RUSTC_WRAPPER cleared");
}

#[cfg(windows)]
#[test]
fn windows_env_guard_leaves_absent_rustc_wrapper_untouched() {
    let _serial_guard = windows_env_guard_test_lock();
    let _vcpkg_root = EnvVarGuard::set("VCPKG_ROOT", r"C:\vcpkg");
    let _rustc_wrapper = EnvVarGuard::remove("RUSTC_WRAPPER");

    run_with_runner("lint", "ui", |_, _| {
        assert_eq!(env::var_os("RUSTC_WRAPPER"), None);
        Ok(())
    })
    .expect("runner should execute without installing RUSTC_WRAPPER");
}

#[cfg(windows)]
#[test]
fn windows_env_guard_test_lock_recovers_after_panic() {
    let result = std::panic::catch_unwind(|| {
        let _serial_guard = windows_env_guard_test_lock();
        panic!("intentionally poison the Windows UI test lock");
    });

    assert!(result.is_err());
    let _serial_guard = windows_env_guard_test_lock();
}

#[cfg(windows)]
fn windows_env_guard_test_lock() -> MutexGuard<'static, ()> {
    // These tests inspect process-global environment state after callbacks,
    // so their whole bodies must run serially rather than only each mutation.
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
