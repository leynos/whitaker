//! End-to-end behaviour of `whitaker-installer run` against a stub `cargo`.
//!
//! These run the real installer binary with a real Clippy snapshot
//! generation in a temporary `XDG_DATA_HOME`, which is why they are Linux
//! only: other platforms do not take the data root from that variable.
#![cfg(target_os = "linux")]

mod bootstrap_support;

use bootstrap_support::{Installation, git};
use std::process::Stdio;
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

#[test]
fn cold_then_warm_runs_build_the_driver_once() {
    let installation = Installation::new();

    let cold = installation.run(&["--all"]);
    assert!(
        cold.status.success(),
        "{}",
        String::from_utf8_lossy(&cold.stderr)
    );
    let warm = installation.run(&["--all"]);
    assert!(
        warm.status.success(),
        "{}",
        String::from_utf8_lossy(&warm.stderr)
    );

    assert_eq!(installation.builds(), 1, "the warm run reuses the driver");
    let notices = |output: &std::process::Output| {
        String::from_utf8_lossy(&output.stderr)
            .matches(BUILT_DRIVER_NOTICE)
            .count()
    };
    assert_eq!(notices(&cold), 1, "the cold run reports its build");
    assert_eq!(notices(&warm), 0, "the warm run builds nothing to report");
}

/// Regression: the build notice once came from the dispatcher's own path
/// choice, so a warm run whose child replaced the driver with identical bytes
/// reported nothing and verification could not see the rebuild.
#[test]
fn a_warm_run_that_replaces_the_driver_reports_the_build() {
    let installation = Installation::new();
    let cold = installation.run(&["--all"]);
    assert!(
        cold.status.success(),
        "{}",
        String::from_utf8_lossy(&cold.stderr)
    );

    let warm = installation
        .run_command(&["--all"])
        .env("STUB_REPLACE_DRIVER", "1")
        .output()
        .expect("run whitaker-installer");

    assert!(
        warm.status.success(),
        "{}",
        String::from_utf8_lossy(&warm.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&warm.stderr)
            .matches(BUILT_DRIVER_NOTICE)
            .count(),
        1,
        "the replaced driver is reported although its bytes are unchanged"
    );
}

#[test]
fn exports_the_bootstrap_and_overwrites_stale_values() {
    let installation = Installation::new();
    let layout = installation.layout();

    let output = installation.run(&["--all"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let recorded = installation.recorded();
    assert_eq!(
        last(&recorded, "CLIPPY"),
        layout.snapshot(&installation.commit).as_str()
    );
    assert_eq!(
        last(&recorded, "DRIVER"),
        layout.driver_dir("6.3.0", &installation.commit).as_str()
    );
    assert_eq!(
        last(&recorded, "LIB"),
        installation.data_root.join("whitaker/lints/lib").as_str()
    );
}

#[test]
fn preserves_arguments_exit_status_and_compiler_wrapper() {
    let installation = Installation::new();

    let output = installation
        .run_command(&["--all", "--", "a spaced argument"])
        .env("STUB_EXIT", "3")
        .env("RUSTC_WRAPPER", "sccache")
        .output()
        .expect("run whitaker-installer");

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
    assert_eq!(last(&recorded, "WRAPPER"), "sccache");
}

#[test]
fn concurrent_cold_runs_build_the_driver_once() {
    let installation = Installation::new();
    let spawn = || {
        installation
            .run_command(&["--all"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn whitaker-installer")
    };

    let (first, second) = (spawn(), spawn());
    let first = first.wait_with_output().expect("first run");
    let second = second.wait_with_output().expect("second run");

    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        installation.builds(),
        1,
        "the lock serializes cold construction"
    );
}

#[test]
fn a_relocated_data_root_keeps_working() {
    let mut installation = Installation::new();
    installation.relocate("moved-data");

    let output = installation.run(&["--all"]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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
fn a_deleted_snapshot_fails_before_cargo_dylint_runs() {
    let installation = Installation::new();
    let snapshot = installation.layout().snapshot(&installation.commit);
    std::fs::remove_dir_all(&snapshot).expect("delete the snapshot generation");

    let output = installation.run(&["--all"]);

    assert!(!output.status.success());
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
fn a_snapshot_that_gained_master_is_rejected() {
    let installation = Installation::new();
    let snapshot = installation.layout().snapshot(&installation.commit);
    git(&snapshot, &["branch", "master", "main"]);

    let output = installation.run(&["--all"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refs/heads/master"), "{stderr}");
    assert!(
        installation.recorded().is_empty(),
        "cargo dylint must not start"
    );
}

#[test]
fn a_different_cargo_dylint_release_is_rejected() {
    let installation = Installation::new();

    let output = installation
        .run_command(&["--all"])
        .env("STUB_DYLINT_VERSION", "6.0.1")
        .output()
        .expect("run whitaker-installer");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reports version 6.0.1"), "{stderr}");
}

#[test]
fn a_missing_installation_points_at_the_installer() {
    let installation = Installation::new();
    std::fs::remove_file(installation.layout().metadata_path()).expect("delete metadata");

    let output = installation.run(&["--all"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("run whitaker-installer"), "{stderr}");
}
