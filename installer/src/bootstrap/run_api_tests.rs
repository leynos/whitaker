//! The readiness and run entry points `whitaker check` drives.
//!
//! Together they are the check port's whole contract: [`preflight`] answers
//! whether this machine can lint without starting Dylint, and [`run_prepared`]
//! runs only the installation that answer approved. The tests pin the parts a
//! caller can observe: no side effects from the query, the child's own status
//! coming back unchanged, and the arguments reaching the child verbatim.

use std::path::{Path, PathBuf};

use rstest::{fixture, rstest};

use super::{fixture::Installed, *};
use crate::dirs::BaseDirs;

/// The file name the recording stub writes its argument list to.
const RECORD_FILE: &str = "cargo-args.txt";

/// Platform directories that know of no data root at all.
struct NoData;

impl BaseDirs for NoData {
    /// Report that the fake deliberately leaves the home directory unset.
    fn home(&self) -> Option<PathBuf> { None }

    /// Report that the fake deliberately leaves the executables directory
    /// unset.
    fn executables(&self) -> Option<PathBuf> { None }

    /// Report that the fake deliberately leaves the data root unset.
    fn whitaker_data(&self) -> Option<PathBuf> { None }
}

/// A recording stub `cargo` and the file it writes its arguments to.
struct RecordingCargo {
    /// The executable stub handed to the prepared run.
    program: PathBuf,
    /// The file the stub records its arguments in.
    record: PathBuf,
}

/// Write an executable stub `cargo` that records its arguments and exits with
/// `code`.
///
/// The stub derives the record path from its own location rather than reading
/// it from the script text: a data root containing a single quote is valid on
/// Unix, but interpolating it into a single-quoted Bash literal would produce
/// an invalid script. `set -e` makes a failed record write fail the stub
/// instead of returning the requested status after losing its arguments.
///
/// # Errors
///
/// Returns the filesystem error when the stub cannot be written or made
/// executable, so each caller reports it as its own failure.
fn recording_cargo(code: i32, directory: &Path) -> std::io::Result<RecordingCargo> {
    use std::os::unix::fs::PermissionsExt;
    let program = directory.join("cargo-recording");
    let record = directory.join(RECORD_FILE);
    let script = format!(
        concat!(
            "#!/usr/bin/env bash\n",
            "set -euo pipefail\n",
            "record=\"$(dirname -- \"$0\")/{}\"\n",
            "printf '%s\\n' \"$@\" > \"$record\"\n",
            "exit {}\n",
        ),
        RECORD_FILE, code
    );
    std::fs::write(&program, script)?;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))?;
    Ok(RecordingCargo { program, record })
}

/// The failure a fixture reports when the machine cannot host the fixture.
///
/// A fixture is shared setup, not a test: it returns the error so the case
/// that needs it fails with the real cause rather than panicking inside
/// `rstest`'s expansion.
type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Build a fresh, complete installation for one test case.
#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn installed() -> FixtureResult<Installed> { Installed::new() }

/// Prepare a run against a fresh installation and its stub `cargo`.
///
/// Returns the installation alongside the prepared run so a case can reach the
/// data root the readiness probe resolved.
#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn prepared_run(installed: FixtureResult<Installed>) -> FixtureResult<(Installed, PreparedRun)> {
    let installation = installed?;
    let cargo = installation.cargo(true)?;
    let prepared = preflight_with_cargo(
        &installation.dirs(),
        cargo.as_os_str(),
        RunOptions::default(),
    )?;
    Ok((installation, prepared))
}

// Assertions live in these helpers because the tests return `Result` to
// propagate fixture errors, and `panic_in_result_fn` denies asserting inside
// such a function.

/// Assert the readiness answer resolved the installation's own snapshot and
/// slot, and left the slot uncreated.
fn assert_prepared_matches(installed: &Installed, prepared: &PreparedRun) {
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

/// Verify a complete installation is approved without creating the slot.
#[rstest]
fn preflight_approves_a_complete_installation_without_touching_it(
    installed: FixtureResult<Installed>,
) -> FixtureResult<()> {
    let installation = installed?;
    let cargo = installation.cargo(true)?;

    let prepared = preflight_with_cargo(
        &installation.dirs(),
        cargo.as_os_str(),
        RunOptions::default(),
    )?;

    assert_prepared_matches(&installation, &prepared);
    Ok(())
}

/// Assert a readiness failure is a missing installation, not a host fault.
fn assert_not_installed(error: &BootstrapError) {
    assert!(
        matches!(error, BootstrapError::NotInstalled { .. }),
        "a missing record is not-installed, not a host fault: {error:?}"
    );
}

/// Verify a missing metadata record is reported as not installed.
#[rstest]
fn preflight_reports_an_installation_that_was_never_made(
    installed: FixtureResult<Installed>,
) -> FixtureResult<()> {
    let installation = installed?;
    std::fs::remove_file(installation.layout.metadata_path())?;

    let error = preflight_with_cargo(
        &installation.dirs(),
        OsStr::new("cargo"),
        RunOptions::default(),
    )
    .expect_err("no metadata means no installation");

    assert_not_installed(&error);
    Ok(())
}

/// Verify platforms without a data directory fail as a host fault.
#[test]
fn preflight_without_platform_directories_is_a_host_fault() {
    let error = preflight(&NoData, RunOptions::default()).expect_err("no data directory");

    assert!(
        matches!(error, BootstrapError::NoDataDirectory),
        "an unknown data directory cannot be repaired by installing: {error:?}"
    );
}

/// Assert the child's own status came back as the run outcome.
fn assert_child_status(finished: RunOutcome, wanted: i32) {
    assert_eq!(
        finished.code, wanted,
        "a lint failure is the child's status, not an error"
    );
}

/// Assert the child received the `dylint` subcommand and Whitaker's arguments.
fn assert_child_arguments(recorded: &str) {
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

/// Assert a spawn failure names the program that could not start.
fn assert_spawn_failure(error: &BootstrapError) {
    assert!(
        matches!(error, BootstrapError::Spawn { .. }),
        "a missing program is a spawn failure: {error:?}"
    );
}

/// Verify the child's exit code comes back unchanged as the run outcome.
#[rstest]
fn run_prepared_returns_the_child_status_unchanged(
    prepared_run: FixtureResult<(Installed, PreparedRun)>,
) -> FixtureResult<()> {
    let (installation, prepared) = prepared_run?;
    let recording = recording_cargo(7, installation.layout.data_root().as_std_path())?;

    let finished = run_prepared_with_cargo(&prepared, recording.program.as_os_str(), &[])
        .expect("the child starts");

    assert_child_status(finished, 7);
    Ok(())
}

/// Verify Whitaker's arguments reach the child after the `dylint` subcommand.
#[rstest]
fn run_prepared_hands_whitakers_arguments_to_the_child(
    prepared_run: FixtureResult<(Installed, PreparedRun)>,
) -> FixtureResult<()> {
    let (installation, prepared) = prepared_run?;
    let recording = recording_cargo(0, installation.layout.data_root().as_std_path())?;

    let arguments: Vec<OsString> = [
        "--lib=whitaker_suite",
        "--no-metadata",
        "--",
        "--all-targets",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    run_prepared_with_cargo(&prepared, recording.program.as_os_str(), &arguments)
        .expect("the child starts");

    let recorded = std::fs::read_to_string(&recording.record)?;
    assert_child_arguments(&recorded);
    Ok(())
}

/// Verify a missing `cargo` program is reported as a spawn failure.
#[rstest]
fn a_missing_cargo_is_reported_rather_than_run(
    prepared_run: FixtureResult<(Installed, PreparedRun)>,
) -> FixtureResult<()> {
    let (installation, prepared) = prepared_run?;
    let missing = installation.layout.data_root().join("no-such-cargo");

    let error = run_prepared_with_cargo(&prepared, missing.as_os_str(), &[])
        .expect_err("there is no cargo to run");

    assert_spawn_failure(&error);
    Ok(())
}
