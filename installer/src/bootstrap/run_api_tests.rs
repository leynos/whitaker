//! The readiness and run entry points `whitaker check` drives.
//!
//! Together they are the check port's whole contract: [`preflight`] answers
//! whether this machine can lint without starting Dylint, and [`run_prepared`]
//! runs only the installation that answer approved. The tests pin the parts a
//! caller can observe: no side effects from the query, the child's own status
//! coming back unchanged, and the arguments reaching the child verbatim.

use std::path::PathBuf;

use super::{fixture::Installed, *};
use crate::dirs::BaseDirs;

/// Platform directories that know of no data root at all.
struct NoData;

impl BaseDirs for NoData {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { None }
}

/// Write an executable stub `cargo` that records its arguments and exits with
/// `code`.
///
/// # Errors
///
/// Returns the filesystem error when the stub cannot be written or made
/// executable, so each caller reports it as its own failure.
fn recording_cargo(code: i32, record: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = record.with_file_name("cargo-recording");
    let script = format!(
        "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > '{}'\nexit {code}\n",
        record.display()
    );
    std::fs::write(&path, script)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(path)
}

#[test]
fn preflight_approves_a_complete_installation_without_touching_it() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");

    let prepared =
        preflight_with_cargo(&installed.dirs(), cargo.as_os_str(), RunOptions::default())
            .expect("a complete installation is ready");

    assert_eq!(
        prepared.snapshot,
        installed.layout.snapshot(&installed.commit),
        "the readiness check must resolve the recorded snapshot"
    );
    assert_eq!(
        prepared.slot.dir(),
        installed.slot_dir(),
        "the readiness check must resolve the slot the run will use"
    );
    assert!(
        !installed.slot_dir().exists(),
        "asking whether the machine can lint must not create the driver slot"
    );
}

#[test]
fn preflight_reports_an_installation_that_was_never_made() {
    let installed = Installed::new().expect("build the installation");
    std::fs::remove_file(installed.layout.metadata_path()).expect("remove the metadata");

    let error = preflight_with_cargo(
        &installed.dirs(),
        OsStr::new("cargo"),
        RunOptions::default(),
    )
    .expect_err("no metadata means no installation");

    assert!(
        matches!(error, BootstrapError::NotInstalled { .. }),
        "a missing record is not-installed, not a host fault: {error:?}"
    );
}

#[test]
fn preflight_without_platform_directories_is_a_host_fault() {
    let error = preflight(&NoData, RunOptions::default()).expect_err("no data directory");

    assert!(
        matches!(error, BootstrapError::NoDataDirectory),
        "an unknown data directory cannot be repaired by installing: {error:?}"
    );
}

#[test]
fn run_prepared_returns_the_child_status_unchanged() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let prepared =
        preflight_with_cargo(&installed.dirs(), cargo.as_os_str(), RunOptions::default())
            .expect("prepare the run");
    let record = installed.layout.data_root().join("args.txt");
    let recording = recording_cargo(7, record.as_std_path()).expect("write the recording cargo");

    let finished =
        run_prepared_with_cargo(&prepared, recording.as_os_str(), &[]).expect("the child starts");

    assert_eq!(
        finished.code, 7,
        "a lint failure is the child's status, not an error"
    );
}

#[test]
fn run_prepared_hands_whitakers_arguments_to_the_child() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let prepared =
        preflight_with_cargo(&installed.dirs(), cargo.as_os_str(), RunOptions::default())
            .expect("prepare the run");
    let record = installed.layout.data_root().join("args.txt");
    let recording = recording_cargo(0, record.as_std_path()).expect("write the recording cargo");

    let arguments: Vec<OsString> = [
        "--lib=whitaker_suite",
        "--no-metadata",
        "--",
        "--all-targets",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    run_prepared_with_cargo(&prepared, recording.as_os_str(), &arguments)
        .expect("the child starts");

    let recorded = std::fs::read_to_string(&record).expect("the child wrote its arguments");
    let received: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        received,
        [
            "dylint",
            "--lib=whitaker_suite",
            "--no-metadata",
            "--",
            "--all-targets"
        ],
        "the prepared run must pass its arguments through after the dylint subcommand"
    );
}

#[test]
fn a_missing_cargo_is_reported_rather_than_run() {
    let installed = Installed::new().expect("build the installation");
    let cargo = installed.cargo(true).expect("write stub cargo");
    let prepared =
        preflight_with_cargo(&installed.dirs(), cargo.as_os_str(), RunOptions::default())
            .expect("prepare the run");
    let missing = installed.layout.data_root().join("no-such-cargo");

    let error = run_prepared_with_cargo(&prepared, missing.as_os_str(), &[])
        .expect_err("there is no cargo to run");

    assert!(
        matches!(error, BootstrapError::Spawn { .. }),
        "a missing program is a spawn failure: {error:?}"
    );
}
