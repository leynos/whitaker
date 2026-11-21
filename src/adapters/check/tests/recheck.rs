//! Which rule the run re-check applies to an approved library path.
//!
//! Readiness approves a library path, and the run re-checks it moments later
//! because the filesystem can change in between. The re-check has to judge the
//! path by the rule that approved it: a recorded directory that lost its suite
//! is the installation's defect and may be repaired, while the user's own path
//! is theirs alone and a change to it must keep the user-path reason. These
//! tests pin that routing, including the case a regression would silently
//! break — a path that was valid when readiness approved it.

#[cfg(unix)]
use std::{ffi::OsStr, path::PathBuf};

use rstest::rstest;
use tempfile::TempDir;
use whitaker_installer::bootstrap::BootstrapError;
#[cfg(unix)]
use whitaker_installer::bootstrap::PreparedRun;

use super::{
    super::suite_rule,
    fixtures::{directory_with_suite, other_suite_filename, suite_filename},
};
#[cfg(unix)]
use super::{
    super::{
        preflight_recorded_with_cargo,
        run::{BootstrappedRun, run_bootstrapped, run_direct_with_cargo},
    },
    fixtures::ReadyInstallation,
};
use crate::{
    adapters::SystemServices,
    ports::{
        CheckPreflightError,
        CheckPrepared,
        CheckRunError,
        CheckServices,
        DirectLibraryPath,
        LibraryRoute,
    },
};

/// A prepared run and the route that approved its library path.
#[cfg(unix)]
struct Approved {
    /// The bootstrap's approved run, including its library path.
    run: PreparedRun,
    /// Which record owns that library path.
    route: LibraryRoute,
}

/// The library shapes a route has to judge.
#[derive(Debug, Clone, Copy)]
enum Shape {
    /// One staged suite, which both routes approve.
    Suite,
    /// No library at all.
    Empty,
    /// Suites for two toolchains, which neither route can choose between.
    TwoSuites,
}

/// Build a directory holding `shape`.
fn directory(shape: Shape) -> TempDir {
    let directory = TempDir::new().expect("a temporary directory is available");
    let names: &[String] = match shape {
        Shape::Suite => &[suite_filename()],
        Shape::Empty => &[],
        Shape::TwoSuites => &[suite_filename(), other_suite_filename()],
    };
    for name in names {
        std::fs::write(directory.path().join(name), b"").expect("the library file can be created");
    }
    directory
}

/// The defect a re-check reported, without the directory path it may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Defect {
    /// The path was approved.
    None,
    /// The recorded directory holds no suite.
    LibrariesMissing,
    /// The recorded directory holds more than one suite.
    RecordedPathAmbiguous,
    /// The user's path holds no suite.
    UserPathHasNoSuite,
    /// The user's path holds more than one suite.
    UserPathAmbiguous,
    /// A defect this table does not expect.
    Other,
}

/// Name the defect `result` reports.
fn defect_of(result: &Result<(), CheckPreflightError>) -> Defect {
    result.as_ref().err().map_or(Defect::None, defect_of_error)
}

/// Name the defect `error` reports.
fn defect_of_error(error: &CheckPreflightError) -> Defect {
    match error {
        CheckPreflightError::Bootstrap(BootstrapError::LibrariesMissing { .. }) => {
            Defect::LibrariesMissing
        }
        CheckPreflightError::RecordedPathAmbiguous { .. } => Defect::RecordedPathAmbiguous,
        CheckPreflightError::UserPathHasNoSuite => Defect::UserPathHasNoSuite,
        CheckPreflightError::UserPathAmbiguous => Defect::UserPathAmbiguous,
        // Every other bootstrap defect is a shape this table does not name.
        CheckPreflightError::Bootstrap(_) => Defect::Other,
    }
}

/// Check that each route judges every library shape by its own rule.
///
/// The two empty cases are the ones that matter: the same absent suite is the
/// installation's defect on one route and the user's on the other, so a route
/// that applied the wrong rule would report the other route's reason.
#[rstest]
#[case::recorded_suite(LibraryRoute::Recorded, Shape::Suite, Defect::None)]
#[case::recorded_empty(LibraryRoute::Recorded, Shape::Empty, Defect::LibrariesMissing)]
#[case::recorded_two(
    LibraryRoute::Recorded,
    Shape::TwoSuites,
    Defect::RecordedPathAmbiguous
)]
#[case::user_suite(LibraryRoute::UserManaged, Shape::Suite, Defect::None)]
#[case::user_empty(LibraryRoute::UserManaged, Shape::Empty, Defect::UserPathHasNoSuite)]
#[case::user_two(LibraryRoute::UserManaged, Shape::TwoSuites, Defect::UserPathAmbiguous)]
fn each_route_applies_its_own_suite_rule(
    #[case] route: LibraryRoute,
    #[case] shape: Shape,
    #[case] expected: Defect,
) {
    let directory = directory(shape);

    let result = suite_rule(route, directory.path().as_os_str());

    assert_eq!(
        defect_of(&result),
        expected,
        "{route:?} judged a {shape:?} path as {result:?}"
    );
}

/// Keep the user-path reason when a user's path changes after readiness.
///
/// The path held exactly one suite when readiness approved it; the suite is
/// removed, or a second one appears, before the run can start. The run must
/// report the user-path reason, not the installation's library defect, because
/// no repair could have changed a directory the user chose. This is the shape
/// a regression would collapse into `LibrariesMissing`, so the composed run is
/// exercised rather than the rule on its own.
#[cfg(unix)]
#[test]
fn a_user_path_that_changed_after_readiness_keeps_the_user_path_reason() {
    let installation = ReadyInstallation::new().expect("the fixture installation is ready");
    let removed = directory_with_suite();
    let duplicated = directory_with_suite();
    let removed_run = approved(&installation, &removed);
    let duplicated_run = approved(&installation, &duplicated);

    std::fs::remove_file(removed.path().join(suite_filename()))
        .expect("the approved suite can be removed");
    std::fs::write(duplicated.path().join(other_suite_filename()), b"")
        .expect("a second suite can appear");

    assert_eq!(
        run_defect(&removed_run),
        Defect::UserPathHasNoSuite,
        "a user path that lost its suite must keep the user-path reason"
    );
    assert_eq!(
        run_defect(&duplicated_run),
        Defect::UserPathAmbiguous,
        "a user path that gained a second suite must keep the user-path reason"
    );
}

/// Refuse a user path that changed before the run, without starting Dylint.
///
/// The direct route's mirror of the bootstrapped re-check: the path held one
/// suite when readiness approved it, the suite is gone by the time the run
/// starts, and the run must report the user-path reason. A route that started
/// Dylint anyway would pass its exit `1` through as a lint denial, and the user
/// would look for a defect in code that was never linted.
#[test]
fn a_user_path_that_changed_before_the_run_is_refused() {
    let directory = directory_with_suite();
    let approved = DirectLibraryPath::new(directory.path().as_os_str().to_owned())
        .expect("a directory holding one suite is approved");

    std::fs::remove_file(directory.path().join(suite_filename()))
        .expect("the approved suite can be removed");

    let result =
        SystemServices::new().check_run(CheckPrepared::Direct(approved), &[], &mut Vec::new());

    assert!(
        matches!(
            result,
            Err(CheckRunError::Preflight(
                CheckPreflightError::UserPathHasNoSuite
            ))
        ),
        "a user path that lost its suite must be refused before Dylint starts, got {result:?}"
    );
}

/// Keep the installation's reason when a recorded directory changes.
///
/// The mirror of the case above: an installation's own directory that loses
/// its suite is the installation's defect, so the run must report it as one.
/// Without this case, a rule that reported every defect as the user's would
/// pass the user-path test while silently disabling the repair.
#[cfg(unix)]
#[test]
fn a_recorded_directory_that_changed_after_readiness_keeps_its_own_reason() {
    let installation = ReadyInstallation::new().expect("the fixture installation is ready");
    let recorded = installation
        .dirs
        .layout()
        .data_root()
        .join("whitaker/lints");
    std::fs::write(recorded.join(suite_filename()), b"").expect("the recorded suite can be staged");
    let prepared = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        None,
        None,
    )
    .expect("a recorded directory holding the suite must be approved");
    let run = bootstrapped(prepared);

    std::fs::remove_file(recorded.join(suite_filename()))
        .expect("the approved suite can be removed");

    assert_eq!(
        run_defect(&run),
        Defect::LibrariesMissing,
        "an installation's own directory that lost its suite is its defect"
    );
}

/// Approve a user-managed library path against a real installation.
#[cfg(unix)]
fn approved(installation: &ReadyInstallation, path: &TempDir) -> Approved {
    let prepared = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        Some(path.path().as_os_str()),
        None,
    )
    .expect("a user path holding the suite must be approved");
    bootstrapped(prepared)
}

/// Keep the approved value split into the run and the route that approved it.
#[cfg(unix)]
fn bootstrapped(prepared: CheckPrepared) -> Approved {
    let CheckPrepared::Bootstrap { run, route } = prepared else {
        panic!("a recorded installation with an override must prepare the bootstrap");
    };
    Approved { run, route }
}

/// Run the re-check and report the defect it found.
#[cfg(unix)]
fn run_defect(approved: &Approved) -> Defect {
    let mut stderr = Vec::new();
    // The run is present only to be refused: a suite rule that approved this
    // path would reach it, and no test here should start Dylint to find out.
    let result = run_bootstrapped(
        BootstrappedRun {
            prepared: &approved.run,
            route: approved.route,
            args: &[],
        },
        &mut stderr,
        |_, _| panic!("the suite rule must refuse before the child is started"),
    );
    assert!(
        stderr.is_empty(),
        "a refused run must not write before it starts: {}",
        String::from_utf8_lossy(&stderr)
    );
    match result {
        Ok(code) => {
            assert_eq!(
                code, 0,
                "the re-check must fail before the child, never run it"
            );
            Defect::None
        }
        Err(CheckRunError::Preflight(error)) => defect_of_error(&error),
        Err(error) => panic!("the re-check must report a preflight defect, got {error:?}"),
    }
}

/// Refuse both changed shapes of a user path before `cargo` starts.
///
/// The direct route has no bootstrap to re-check for it, so its own check is
/// all that stands between an approved path and Dylint. Both invalid shapes
/// are covered: a check that noticed only a missing suite would still let an
/// ambiguous path run Dylint and pass its exit `1` through as a lint denial.
///
/// The stub records its own start, so the assertion is about the child rather
/// than about the error the check returned; a test that only read the error
/// would pass against a route that spawned first and then refused.
#[cfg(unix)]
#[test]
fn a_user_path_that_stopped_naming_one_suite_is_refused_before_cargo_starts() {
    let removed = directory_with_suite();
    let approved_removed = DirectLibraryPath::new(removed.path().as_os_str().to_owned())
        .expect("a directory holding one suite is approved");
    std::fs::remove_file(removed.path().join(suite_filename()))
        .expect("the approved suite can be removed");

    let duplicated = directory_with_suite();
    let approved_duplicated = DirectLibraryPath::new(duplicated.path().as_os_str().to_owned())
        .expect("a directory holding one suite is approved");
    std::fs::write(duplicated.path().join(other_suite_filename()), b"")
        .expect("a second suite can appear");

    let cargo = RecordingCargo::new();

    for (approved, expected) in [
        (approved_removed, Defect::UserPathHasNoSuite),
        (approved_duplicated, Defect::UserPathAmbiguous),
    ] {
        let result = run_direct_with_cargo(approved.as_os_str(), &[], cargo.program());
        let defect = match &result {
            Err(CheckRunError::Preflight(error)) => defect_of_error(error),
            _ => Defect::Other,
        };
        assert_eq!(
            defect, expected,
            "a user path that stopped naming one suite must be refused, got {result:?}"
        );
    }
    assert!(
        !cargo.was_started(),
        "a refused path must be judged before cargo is started"
    );
}

/// A `cargo` stub that records its own start, so its absence is provable.
///
/// The record has to be written by the child: a flag in the test process would
/// be set whether or not the command ran. A POSIX shell script is enough for
/// the same reason the readiness stub is one, and its test is Unix-only; the
/// platform-neutral refusal is pinned by
/// `a_user_path_that_changed_before_the_run_is_refused`.
#[cfg(unix)]
struct RecordingCargo {
    /// Owns the directory the stub lives in.
    _directory: TempDir,
    /// The program the run is asked to start.
    program: PathBuf,
    /// The file the stub writes when it starts.
    log: PathBuf,
}

#[cfg(unix)]
impl RecordingCargo {
    /// Write a stub that appends a line to its log and exits successfully.
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = TempDir::new().expect("a temporary directory is available");
        let log = directory.path().join("stub.log");
        let program = directory.path().join("cargo");
        let script = format!(
            "#!/bin/sh\nprintf 'started\\n' >> '{}'\nexit 0\n",
            log.display()
        );
        std::fs::write(&program, script).expect("the stub cargo can be written");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("the stub cargo can be made executable");
        Self {
            _directory: directory,
            program,
            log,
        }
    }

    /// The program the run is asked to start.
    fn program(&self) -> &OsStr { self.program.as_os_str() }

    /// Whether the stub was started.
    fn was_started(&self) -> bool { self.log.exists() }
}
