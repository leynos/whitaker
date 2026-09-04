//! Tests that verify the UI harness runner validates inputs and propagates
//! errors from custom runners.
use super::{HarnessError, run_ui_test, run_with_runner};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use std::env;
use whitaker_common::test_support::{EnvTestGuard, EnvVarGuard, env_test_guard};

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
    let _serial_guard = runner_env_guard_test_lock();

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

#[test]
fn run_ui_test_panics_with_context_from_runner_failure() {
    let panic = std::panic::catch_unwind(|| {
        run_ui_test("lint", "ui", |_, _| {
            Err(String::from("known runner failure"))
        });
    })
    .expect_err("runner failure should panic at the UI test boundary");
    let message = panic
        .downcast::<String>()
        .map(|message| *message)
        .expect("the formatted UI-test panic should carry a string message");

    assert!(message.contains("lint"));
    assert!(message.contains("ui"));
    assert!(message.contains("known runner failure"));
}

#[test]
fn runner_env_guard_clears_and_restores_rustc_wrapper() {
    let _serial_guard = runner_env_guard_test_lock();
    let _guard = EnvVarGuard::set("RUSTC_WRAPPER", "sccache");

    run_with_runner("lint", "ui", |_, _| {
        assert_eq!(env::var_os("RUSTC_WRAPPER"), None);
        Ok(())
    })
    .expect("runner should execute with RUSTC_WRAPPER cleared");

    assert_eq!(env::var_os("RUSTC_WRAPPER"), Some("sccache".into()));
}

#[cfg(windows)]
#[test]
fn windows_env_guard_leaves_absent_rustc_wrapper_untouched() {
    let _serial_guard = runner_env_guard_test_lock();
    let _vcpkg_root = EnvVarGuard::set("VCPKG_ROOT", r"C:\vcpkg");
    let _rustc_wrapper = EnvVarGuard::remove("RUSTC_WRAPPER");

    run_with_runner("lint", "ui", |_, _| {
        assert_eq!(env::var_os("RUSTC_WRAPPER"), None);
        Ok(())
    })
    .expect("runner should execute without installing RUSTC_WRAPPER");

    assert_eq!(env::var_os("RUSTC_WRAPPER"), None);
}

#[test]
fn runner_env_guard_test_lock_releases_after_panic() {
    let result = std::panic::catch_unwind(|| {
        let _serial_guard = runner_env_guard_test_lock();
        panic!("intentionally release the UI environment test lock");
    });

    assert!(result.is_err());
    let (acquired_sender, acquired_receiver) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let _serial_guard = runner_env_guard_test_lock();
        acquired_sender
            .send(())
            .expect("second thread must report lock acquisition");
    });

    acquired_receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("second thread should acquire the released environment lock");
    thread
        .join()
        .expect("second environment-lock test thread should complete");
}

/// Acquires the production environment protocol for assertions spanning a callback.
fn runner_env_guard_test_lock() -> EnvTestGuard {
    // These tests inspect process-global environment state after callbacks,
    // so their whole bodies must run serially rather than only each mutation.
    env_test_guard()
}
