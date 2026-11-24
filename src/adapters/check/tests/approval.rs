//! What readiness keeps for the run that consumes its answer.
//!
//! The port's prepared value names only the route, so the concrete run is held
//! by the adapter between readiness and the run. These tests pin the two
//! promises that keep "Dylint runs only against a freshly approved
//! installation" intact now that the port value can be copied: a run with no
//! held approval starts nothing, and one approval arms one run.

use super::super::ApprovedRuns;
#[cfg(unix)]
use super::{
    super::{Approval, preflight_recorded_with_cargo, prepare_direct_library_path},
    fixtures::{ReadyInstallation, directory_with_suite, suite_filename},
};
use crate::ports::{ApprovalId, CheckRunError};
#[cfg(unix)]
use crate::ports::{CheckPrepared, LibraryRoute};

/// Approve a real recorded installation and keep the run the adapter keeps.
///
/// `ReadyInstallation` installs the installer's genuine snapshot bundle and a
/// stub `cargo`, which only a Unix host can stage, so every test that needs a
/// real approval is Unix-gated. The holder's own promises are tested by
/// `a_run_without_an_approval_is_reported_rather_than_started` on every host.
#[cfg(unix)]
fn held_approval() -> (ReadyInstallation, ApprovedRuns, ApprovalId) {
    let installation = ReadyInstallation::new().expect("the fixture installation is ready");
    let recorded = installation
        .dirs
        .layout()
        .data_root()
        .join("whitaker/lints");
    std::fs::write(recorded.join(suite_filename()), b"").expect("the recorded suite can be staged");
    let answer = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        None,
        None,
    )
    .expect("a recorded directory holding the suite must be approved");
    let held = ApprovedRuns::new();

    let prepared = answer.record(&held);
    let CheckPrepared::Bootstrap { approval, .. } = prepared else {
        panic!("a recorded installation must approve the bootstrap route");
    };
    (installation, held, approval)
}

/// Report the route a prepared value names, when it approved a bootstrap.
///
/// The prepared value deliberately carries no installer type, so a test reads
/// the route it names rather than the run behind it.
#[cfg(unix)]
fn route_of(prepared: &CheckPrepared) -> Option<LibraryRoute> {
    match prepared {
        CheckPrepared::Bootstrap { route, .. } => Some(*route),
        CheckPrepared::Direct(_) => None,
    }
}

/// Keep the concrete run out of the port's prepared value.
///
/// The value the command layer sees names the route and nothing else, so the
/// installer's own run has to stay with the adapter that inspected it. That is
/// what the held slot is for, and what makes the run available here.
#[cfg(unix)]
#[test]
fn an_approved_installation_is_held_rather_than_returned() {
    let (_installation, held, approval) = held_approval();

    let run = held.take(approval);

    assert!(
        run.is_ok(),
        "readiness must keep the run its approval covers, got {run:?}"
    );
}

/// Arm exactly one run from one approval.
///
/// One approval is evidence for one run. Without the take, a copied prepared
/// value would let a second run start Dylint against an installation nobody
/// re-approved, which is the guarantee the port value no longer carries.
#[cfg(unix)]
#[test]
fn consuming_an_approval_leaves_nothing_for_a_second_run() {
    let (_installation, held, approval) = held_approval();

    assert!(
        held.take(approval).is_ok(),
        "the held approval must be available to the run that consumes it"
    );
    assert!(
        matches!(held.take(approval), Err(CheckRunError::Unapproved)),
        "a consumed approval must not arm a second run"
    );
}

/// Report a run that began with no approval, rather than running anyway.
///
/// The shape a caller produces by skipping readiness: the port value can be
/// constructed from the route alone, so the run must refuse rather than start
/// Dylint against an installation nothing inspected.
#[test]
fn a_run_without_an_approval_is_reported_rather_than_started() {
    let held = ApprovedRuns::new();

    assert!(
        matches!(
            held.take(ApprovalId::new(1)),
            Err(CheckRunError::Unapproved)
        ),
        "a run with no approved installation must be reported"
    );
}

/// Refuse a stale approval without consuming the current one.
///
/// A caller that answers readiness twice holds two values, and running the
/// earlier one must not destroy the run the later answer still covers. The
/// comparison therefore happens before the run is taken: a refusal leaves the
/// slot intact, and the current approval can still run afterwards.
#[cfg(unix)]
#[test]
fn refusing_a_stale_approval_leaves_the_current_one_runnable() {
    let (_installation, held, current) = held_approval();
    let stale = ApprovalId::new(current.number() - 1);

    assert!(
        matches!(held.take(stale), Err(CheckRunError::Unapproved)),
        "a value from an earlier preflight must be refused"
    );
    assert!(
        held.take(current).is_ok(),
        "refusing the stale approval must leave the current run available"
    );
}

/// Drop a stale approval when the route changes to the user's own path.
///
/// Readiness may approve a bootstrap installation and a later answer take the
/// direct route. Leaving the earlier run in place would let a subsequent
/// bootstrap run consume an approval the flow has already abandoned.
#[cfg(unix)]
#[test]
fn a_direct_approval_clears_a_run_an_earlier_answer_held() {
    let (_installation, held, _approval) = held_approval();
    let directory = directory_with_suite();
    let path = prepare_direct_library_path(directory.path().as_os_str().to_owned())
        .expect("the staged suite makes the path ready");

    let recorded = Approval::Direct(path).record(&held);

    assert!(
        matches!(recorded, CheckPrepared::Direct(_)),
        "a user-managed path must approve the direct route"
    );
    assert!(
        matches!(
            held.take(ApprovalId::new(1)),
            Err(CheckRunError::Unapproved)
        ),
        "the abandoned bootstrap approval must not survive a direct answer"
    );
}

/// Report the user-managed route when the user's path overrode the record.
///
/// The bootstrap still supplies the snapshot and driver, so the approval is a
/// bootstrap one; only the rule that judges the path changes, and the run
/// re-check has to keep applying the user-path rule.
#[cfg(unix)]
#[test]
fn a_user_managed_override_approves_the_user_managed_route() {
    let installation = ReadyInstallation::new().expect("the fixture installation is ready");
    let directory = directory_with_suite();
    let approval = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        Some(directory.path().as_os_str()),
        None,
    )
    .expect("a user path holding the suite must be approved");
    let held = ApprovedRuns::new();

    assert_eq!(
        route_of(&approval.record(&held)),
        Some(LibraryRoute::UserManaged),
        "an override must approve the route the user's own path takes"
    );
}
