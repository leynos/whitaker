//! End-to-end behaviour of `whitaker-installer run` against a stub `cargo`.
//!
//! These run the real installer binary with a real Clippy snapshot
//! generation in a temporary `XDG_DATA_HOME`, which is why they are Linux
//! only: other platforms do not take the data root from that variable.
#![cfg(target_os = "linux")]

mod bootstrap_support;

use std::process::{Output, Stdio};

use bootstrap_support::{Installation, TOOLCHAIN, git};
use whitaker_installer::bootstrap::BUILT_DRIVER_NOTICE;

/// The value the stub recorded for `key`, from its most recent invocation.
fn last(recorded: &[String], key: &str) -> String {
    let prefix = format!("{key}=");
    recorded
        .iter()
        .rev()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("the stub recorded no {key}"))
        .to_owned()
}

/// Assert a run succeeded, reporting its stderr when it did not.
fn assert_succeeded(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Assert the stub performed `expected` cold builds.
fn assert_builds(installation: &Installation, expected: usize, message: &str) {
    assert_eq!(installation.builds(), expected, "{message}");
}

#[test]
fn cold_then_warm_runs_build_the_driver_once() -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;

    let cold = installation.run(&["--all"])?;
    assert_succeeded(&cold);
    let warm = installation.run(&["--all"])?;
    assert_succeeded(&warm);

    assert_driver_reused(&installation, &cold, &warm);
    Ok(())
}

/// Check the cold run built once and the warm run reused the driver.
fn assert_driver_reused(installation: &Installation, cold: &Output, warm: &Output) {
    assert_eq!(installation.builds(), 1, "the warm run reuses the driver");
    let notices = |output: &Output| {
        String::from_utf8_lossy(&output.stderr)
            .matches(BUILT_DRIVER_NOTICE)
            .count()
    };
    assert_eq!(notices(cold), 1, "the cold run reports its build");
    assert_eq!(notices(warm), 0, "the warm run builds nothing to report");
}

/// Regression: the build notice once came from the dispatcher's own path
/// choice, so a warm run whose child replaced the driver with identical bytes
/// reported nothing and verification could not see the rebuild.
#[test]
fn a_warm_run_that_replaces_the_driver_reports_the_build() -> Result<(), Box<dyn std::error::Error>>
{
    let installation = Installation::new()?;
    let cold = installation.run(&["--all"])?;
    assert_succeeded(&cold);

    let warm = installation
        .run_command(&["--all"])
        .env("STUB_REPLACE_DRIVER", "1")
        .output()?;

    assert_succeeded(&warm);
    assert_replacement_reported(&warm);
    Ok(())
}

/// Check that replacing the driver with identical bytes is reported.
fn assert_replacement_reported(warm: &Output) {
    assert_eq!(
        String::from_utf8_lossy(&warm.stderr)
            .matches(BUILT_DRIVER_NOTICE)
            .count(),
        1,
        "the replaced driver is reported although its bytes are unchanged"
    );
}

/// A warm run's rebuild reads the snapshot without the cold path's full
/// verification, so the installer verifies it afterwards: a snapshot that
/// fails fails the run and leaves the slot cold.
#[test]
fn a_warm_rebuild_from_an_unverifiable_snapshot_fails_and_goes_cold()
-> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;
    let cold = installation.run(&["--all"])?;
    assert_succeeded(&cold);
    let snapshot = installation.layout().snapshot(&installation.commit);
    // Full verification refuses remotes; the cheaper installed check does not.
    git(
        &snapshot,
        &[
            "remote",
            "add",
            "upstream",
            "https://example.invalid/clippy",
        ],
    )?;

    let warm = installation
        .run_command(&["--all"])
        .env("STUB_REPLACE_DRIVER", "1")
        .output()?;
    assert_unverified_rebuild_refused(&warm);

    let next = installation.run(&["--all"])?;
    assert_slot_is_cold(&next);
    Ok(())
}

/// Check that a warm rebuild from a snapshot with remotes is refused.
fn assert_unverified_rebuild_refused(warm: &Output) {
    assert!(
        !warm.status.success(),
        "the unverified rebuild must not succeed"
    );
    let stderr = String::from_utf8_lossy(&warm.stderr);
    assert!(stderr.contains("it has remotes"), "{stderr}");
}

/// Check that the refused rebuild left the slot cold.
fn assert_slot_is_cold(next: &Output) {
    assert!(
        !next.status.success(),
        "the slot is cold and verification refuses it"
    );
    assert!(
        String::from_utf8_lossy(&next.stderr).contains("it has remotes"),
        "{}",
        String::from_utf8_lossy(&next.stderr)
    );
}

/// A cold run removes an unmarked driver, so it builds from the snapshot it
/// has just verified rather than adopting a leftover.
#[test]
fn a_cold_run_rebuilds_an_unmarked_driver() -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;
    let first = installation.run(&["--all"])?;
    assert_succeeded(&first);
    let slot = installation
        .layout()
        .driver_dir("6.3.0", &installation.commit);
    std::fs::remove_file(slot.join(format!(".{TOOLCHAIN}.ready")))?;

    let second = installation.run(&["--all"])?;
    assert_succeeded(&second);

    assert_builds(&installation, 2, "the unmarked driver is rebuilt");
    Ok(())
}

/// Replace the ready driver with identical bytes, as a concurrent warm run's
/// rebuild would, leaving the marker naming the driver it replaced. This is
/// the state between that run's rebuild and its verification of the snapshot.
///
/// # Errors
///
/// Returns an error if the replacement driver cannot be written or moved into
/// place.
fn replace_driver_mid_verification(
    installation: &Installation,
) -> Result<(), Box<dyn std::error::Error>> {
    let toolchain_dir = installation
        .layout()
        .driver_dir("6.3.0", &installation.commit)
        .join(TOOLCHAIN);
    let partial = toolchain_dir.join("dylint-driver.partial");
    std::fs::write(&partial, "#!/bin/sh\n")?;
    std::fs::rename(&partial, toolchain_dir.join("dylint-driver"))?;
    Ok(())
}

/// Regression: a run that started while another warm run was still verifying
/// its rebuild used to accept the unverified driver. It must take the cold
/// path instead, and so fail when the snapshot behind the rebuild is bad.
#[rstest::rstest]
#[case::snapshot_fails_verification(true)]
#[case::snapshot_verifies(false)]
fn a_run_never_uses_a_rebuild_another_run_has_not_verified(
    #[case] is_corrupt: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;
    let cold = installation.run(&["--all"])?;
    assert_succeeded(&cold);
    if is_corrupt {
        let snapshot = installation.layout().snapshot(&installation.commit);
        git(
            &snapshot,
            &[
                "remote",
                "add",
                "upstream",
                "https://example.invalid/clippy",
            ],
        )?;
    }
    replace_driver_mid_verification(&installation)?;

    let next = installation.run(&["--all"])?;

    assert_unverified_driver_not_used(is_corrupt, &installation, &next);
    Ok(())
}

/// Check the run rebuilt rather than adopting an unverified driver.
fn assert_unverified_driver_not_used(is_corrupt: bool, installation: &Installation, next: &Output) {
    let stderr = String::from_utf8_lossy(&next.stderr);
    if is_corrupt {
        assert!(
            !next.status.success(),
            "the unverified driver was used: {stderr}"
        );
        assert!(stderr.contains("it has remotes"), "{stderr}");
    } else {
        assert!(next.status.success(), "{stderr}");
        assert_eq!(installation.builds(), 2, "the unverified driver is rebuilt");
        assert_eq!(stderr.matches(BUILT_DRIVER_NOTICE).count(), 1, "{stderr}");
    }
}

#[test]
fn exports_the_bootstrap_and_overwrites_stale_values() -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;

    let output = installation.run(&["--all"])?;
    assert_succeeded(&output);
    assert_bootstrap_env(&installation);

    Ok(())
}

/// Check the child saw the bootstrap paths, not the stale values.
fn assert_bootstrap_env(installation: &Installation) {
    let layout = installation.layout();
    let recorded = installation.recorded();
    assert_eq!(
        last(&recorded, "CLIPPY"),
        layout.snapshot(&installation.commit).as_str(),
        "the child should see the installed snapshot path"
    );
    assert_eq!(
        last(&recorded, "DRIVER"),
        layout.driver_dir("6.3.0", &installation.commit).as_str(),
        "the child should see the installed driver path"
    );
    assert_eq!(
        last(&recorded, "LIB"),
        installation.data_root.join("whitaker/lints/lib").as_str(),
        "the child should see the built lint library path"
    );
}

#[test]
fn preserves_arguments_exit_status_and_compiler_wrapper() -> Result<(), Box<dyn std::error::Error>>
{
    let installation = Installation::new()?;

    let output = installation
        .run_command(&["--all", "--", "a spaced argument"])
        .env("STUB_EXIT", "3")
        .env("RUSTC_WRAPPER", "sccache")
        .output()?;

    assert_arguments_preserved(&installation, &output);
    Ok(())
}

/// Check the child received the arguments, exit status, and wrapper.
fn assert_arguments_preserved(installation: &Installation, output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(3),
        "lint failures keep their exit code"
    );
    let recorded = installation.recorded();
    assert!(
        recorded.contains(&"ARG=a spaced argument".to_owned()),
        "{recorded:?}"
    );
    assert_eq!(
        last(&recorded, "WRAPPER"),
        "sccache",
        "the compiler wrapper should reach the child unchanged"
    );
}

#[test]
fn concurrent_cold_runs_build_the_driver_once() -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;
    let spawn = || {
        installation
            .run_command(&["--all"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn whitaker-installer")
    };

    let (first_child, second_child) = (spawn(), spawn());
    let first = first_child.wait_with_output()?;
    let second = second_child.wait_with_output()?;

    assert_succeeded(&first);
    assert_succeeded(&second);
    assert_builds(&installation, 1, "the lock serializes cold construction");
    Ok(())
}

#[test]
fn a_relocated_data_root_keeps_working() -> Result<(), Box<dyn std::error::Error>> {
    let mut installation = Installation::new()?;
    installation.relocate("moved-data")?;

    let output = installation.run(&["--all"])?;
    assert_succeeded(&output);

    assert_paths_follow_move(&installation);
    Ok(())
}

/// Check the recorded bootstrap paths followed the moved data root.
fn assert_paths_follow_move(installation: &Installation) {
    let moved = installation.data_root.as_str();
    let recorded = installation.recorded();
    for key in ["CLIPPY", "DRIVER", "LIB"] {
        assert!(
            last(&recorded, key).starts_with(moved),
            "{key} must follow the move"
        );
    }
}

#[test]
fn a_deleted_snapshot_fails_before_cargo_dylint_runs() -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;
    let snapshot = installation.layout().snapshot(&installation.commit);
    std::fs::remove_dir_all(&snapshot)?;

    let output = installation.run(&["--all"])?;

    assert_deleted_snapshot_refused(&installation, &output);
    Ok(())
}

/// Check a deleted snapshot fails before `cargo dylint` starts.
fn assert_deleted_snapshot_refused(installation: &Installation, output: &Output) {
    assert!(
        !output.status.success(),
        "a deleted snapshot should fail the run"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("is not installed"), "{stderr}");
    assert!(
        stderr.contains("never falls back to cloning public Clippy"),
        "{stderr}"
    );
    assert!(
        installation.recorded().is_empty(),
        "cargo dylint must not start"
    );
}

#[test]
fn a_snapshot_that_gained_master_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;
    let snapshot = installation.layout().snapshot(&installation.commit);
    git(&snapshot, &["branch", "master", "main"])?;

    let output = installation.run(&["--all"])?;

    assert_master_branch_refused(&installation, &output);
    Ok(())
}

/// Check a snapshot that gained a `master` ref is rejected.
fn assert_master_branch_refused(installation: &Installation, output: &Output) {
    assert!(
        !output.status.success(),
        "a snapshot with a master ref should fail the run"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refs/heads/master"), "{stderr}");
    assert!(
        installation.recorded().is_empty(),
        "cargo dylint must not start"
    );
}

#[test]
fn a_different_cargo_dylint_release_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;

    let output = installation
        .run_command(&["--all"])
        .env("STUB_DYLINT_VERSION", "6.0.1")
        .output()?;

    assert_wrong_dylint_version_refused(&output);
    Ok(())
}

/// Check a different `cargo dylint` release is rejected.
fn assert_wrong_dylint_version_refused(output: &Output) {
    assert!(
        !output.status.success(),
        "a mismatched cargo dylint release should fail the run"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reports version 6.0.1"), "{stderr}");
}

#[test]
fn a_missing_installation_points_at_the_installer() -> Result<(), Box<dyn std::error::Error>> {
    let installation = Installation::new()?;
    std::fs::remove_file(installation.layout().metadata_path())?;

    let output = installation.run(&["--all"])?;

    assert_missing_installation_reported(&output);
    Ok(())
}

/// Check a missing installation tells the user to run the installer.
fn assert_missing_installation_reported(output: &Output) {
    assert!(
        !output.status.success(),
        "a missing installation should fail the run"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("run whitaker-installer"), "{stderr}");
}
