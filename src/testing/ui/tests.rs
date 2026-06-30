//! Tests that verify the UI harness runner validates inputs and propagates
//! errors from custom runners.
use super::{HarnessError, run_with_runner};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use std::env;
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use whitaker_common::test_support::env_test_guard;

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
    let _guard = EnvVarGuard::set("RUSTC_WRAPPER", "sccache");

    run_with_runner("lint", "ui", |_, _| {
        assert_eq!(env::var_os("RUSTC_WRAPPER"), None);
        Ok(())
    })
    .expect("runner should execute with RUSTC_WRAPPER cleared");

    assert_eq!(
        env::var_os("RUSTC_WRAPPER"),
        Some(OsString::from("sccache"))
    );
}

#[cfg(windows)]
struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

#[cfg(windows)]
impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let _env_guard = env_test_guard();
        let previous = env::var_os(key);
        // SAFETY: `env_test_guard` serializes this environment mutation.
        unsafe {
            env::set_var(key, value);
        }
        Self { key, previous }
    }
}

#[cfg(windows)]
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        let _env_guard = env_test_guard();
        match &self.previous {
            Some(previous) => {
                // SAFETY: `env_test_guard` serializes this environment mutation.
                unsafe {
                    env::set_var(self.key, previous);
                }
            }
            None => {
                // SAFETY: `env_test_guard` serializes this environment mutation.
                unsafe {
                    env::remove_var(self.key);
                }
            }
        }
    }
}
