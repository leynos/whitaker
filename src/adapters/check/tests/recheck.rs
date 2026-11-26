//! Which rule the run re-check applies to an approved library path.
//!
//! Readiness approves a library path, and the run re-checks it moments later
//! because the filesystem can change in between. The re-check has to judge the
//! path by the rule that approved it: a recorded directory that lost its suite
//! is the installation's defect and may be repaired, while the user's own path
//! is theirs alone and a change to it must keep the user-path reason. These
//! tests pin that routing, including the case a regression would silently
//! break — a path that was valid when readiness approved it.

use rstest::rstest;
use tempfile::TempDir;
#[cfg(unix)]
use whitaker_installer::bootstrap::PreparedRun;

#[cfg(unix)]
use super::super::approval::Approval;
use super::{
    super::suite_rule,
    fixtures::{
        RecordingCargo,
        directory_with_suite,
        other_suite_filename,
        prepared_direct,
        suite_filename,
    },
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
        BootstrapFault,
        CheckPreflightError,
        CheckPrepared,
        CheckRunError,
        CheckServices,
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
        CheckPreflightError::Bootstrap(BootstrapFault::LibrariesMissing { .. }) => {
            Defect::LibrariesMissing
        }
        CheckPreflightError::RecordedPathAmbiguous { .. } => Defect::RecordedPathAmbiguous,
        CheckPreflightError::UserPathHasNoSuite => Defect::UserPathHasNoSuite,
        CheckPreflightError::UserPathAmbiguous => Defect::UserPathAmbiguous,
        // Every other bootstrap or path defect is a shape this table does not
        // name.
        CheckPreflightError::Bootstrap(_)
        | CheckPreflightError::UserPathUnusable { .. }
        | CheckPreflightError::RecordedPathUnusable { .. } => Defect::Other,
    }
}

/// The run, route and approval an answer carries, taken the way the adapter does.
///
/// The approval is recorded through the adapter's own holder and then taken
/// back through it, so the test consumes the value exactly as `check_run`
/// would rather than destructuring the adapter's private enum.
#[cfg(unix)]
fn approved_run(approval: Approval) -> Approved {
    use crate::ports::CheckPrepared;

    let held = super::super::ApprovedRuns::new();
    let prepared = approval.record(&held);
    let CheckPrepared::Bootstrap {
        route,
        approval: approval_id,
    } = prepared
    else {
        panic!("a recorded installation with an override must prepare the bootstrap");
    };
    let run = held
        .take(approval_id)
        .expect("the adapter must keep the run its own approval covers");
    Approved { run, route }
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

/// Refuse a direct value whose approval the adapter never issued.
///
/// A `Direct` prepared value can be built from a validated path alone, so the
/// only thing standing between a hand-built value and a run is the approval
/// number it carries. The adapter refuses one it never issued, which is why
/// the re-check itself is exercised through `run_direct_with_cargo` in
/// `a_user_path_that_stopped_naming_one_suite_is_refused_before_cargo_starts`.
#[test]
fn a_direct_value_the_adapter_did_not_issue_is_refused() {
    let directory = directory_with_suite();
    let approved = prepared_direct(directory.path());
    let prepared = CheckPrepared::direct(approved);

    let result = SystemServices::new().check_run(prepared, &[], &mut Vec::new());

    assert!(
        matches!(result, Err(CheckRunError::Unapproved)),
        "a value the adapter never issued must not reach Dylint, got {result:?}"
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
    let approval = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        None,
        None,
    )
    .expect("a recorded directory holding the suite must be approved");
    let run = approved_run(approval);

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
    let approval = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        Some(path.path().as_os_str()),
        None,
    )
    .expect("a user path holding the suite must be approved");
    approved_run(approval)
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
    let approved_removed = prepared_direct(removed.path());
    std::fs::remove_file(removed.path().join(suite_filename()))
        .expect("the approved suite can be removed");

    let duplicated = directory_with_suite();
    let approved_duplicated = prepared_direct(duplicated.path());
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
        cargo.starts() == 0,
        "a refused path must be judged before cargo is started"
    );
}
