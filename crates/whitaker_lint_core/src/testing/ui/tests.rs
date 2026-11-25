//! Tests that verify the UI harness runner validates inputs and propagates
//! errors from custom runners.
use std::env;

use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

use super::{HarnessError, describe_panic, run_ui_test, run_with_runner, toolchain};

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

/// The harness no longer sets the copy-library flag on the process: it reaches
/// the build commands instead.
///
/// Regression: without the flag, Dylint 6.3.0's `dylint_testing` copies the
/// default-feature stub over the harness-built library, and the UI test fails
/// with ``could not find `dylint_version` ``.
#[test]
fn the_runner_leaves_the_process_environment_alone() {
    const FLAG: &str = "DYLINT_LINK_ENABLE_COPY_LIBRARY";
    let before = env::var_os(FLAG);

    run_with_runner("lint", "ui", |_, _| {
        assert_eq!(env::var_os(FLAG), before);
        Ok(())
    })
    .expect("runner should execute without changing the environment");

    assert_eq!(env::var_os(FLAG), before);
}

#[rstest]
#[case::absent_is_supplied(false, true)]
#[case::explicit_value_is_kept(true, false)]
fn build_env_defaults_supply_the_copy_flag_only_when_absent(
    #[case] caller_sets_it: bool,
    #[case] expect_supplied: bool,
) {
    const FLAG: &str = "DYLINT_LINK_ENABLE_COPY_LIBRARY";

    let defaults = toolchain::build_env_defaults(|key| caller_sets_it && key == FLAG);

    assert_eq!(defaults.contains(&(FLAG, "1")), expect_supplied);
}

#[rstest]
#[case::string(Box::new(String::from("formatted")), "formatted")]
#[case::static_str(Box::new("literal"), "literal")]
#[case::other(Box::new(7_u8), "dylint UI tests panicked without a message")]
fn describe_panic_reads_the_payload(
    #[case] payload: Box<dyn std::any::Any + Send>,
    #[case] expected: &str,
) {
    assert_eq!(describe_panic(payload), expected);
}
