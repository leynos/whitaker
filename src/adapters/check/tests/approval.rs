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
#[cfg(unix)]
use crate::adapters::SystemServices;
#[cfg(unix)]
use crate::ports::CheckServices;
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
        CheckPrepared::Direct { .. } => None,
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
    let CheckPrepared::Direct { approval, .. } = recorded else {
        panic!("a user-managed path must approve the direct route");
    };

    assert!(
        matches!(
            held.take(ApprovalId::new(1)),
            Err(CheckRunError::Unapproved)
        ),
        "the abandoned bootstrap approval must not survive a direct answer"
    );
    assert!(
        held.consume(approval).is_ok(),
        "the direct approval itself must be the newest answer"
    );
}

/// Refuse an earlier direct approval once a later one has answered.
///
/// Direct answers hold no run, so the only thing that can make a stale one
/// refuse is the approval number. Without it, two direct values from one
/// adapter would both be runnable and the supersession rule would hold on the
/// bootstrap route only.
#[cfg(unix)]
#[test]
fn a_superseded_direct_approval_is_refused() {
    let held = ApprovedRuns::new();
    let first_directory = directory_with_suite();
    let second_directory = directory_with_suite();
    let first = Approval::Direct(
        prepare_direct_library_path(first_directory.path().as_os_str().to_owned())
            .expect("the first staged suite makes its path ready"),
    )
    .record(&held);
    let second = Approval::Direct(
        prepare_direct_library_path(second_directory.path().as_os_str().to_owned())
            .expect("the second staged suite makes its path ready"),
    )
    .record(&held);
    let (
        CheckPrepared::Direct {
            approval: first_id, ..
        },
        CheckPrepared::Direct {
            approval: second_id,
            ..
        },
    ) = (first, second)
    else {
        panic!("both answers must approve the direct route");
    };

    assert!(
        matches!(held.consume(first_id), Err(CheckRunError::Unapproved)),
        "the superseded direct approval must be refused"
    );
    assert!(
        held.consume(second_id).is_ok(),
        "the newest direct approval must still be consumable"
    );
}

/// Refuse a direct approval once a later bootstrap answer has answered.
///
/// The bootstrap answer replaces the direct one, so a direct value left over
/// from before it must not run: a caller that ignored the newer answer would
/// otherwise lint with a path readiness had already superseded.
#[cfg(unix)]
#[test]
fn a_direct_approval_superseded_by_a_bootstrap_answer_is_refused() {
    let (installation, held, _bootstrap) = held_approval();
    let directory = directory_with_suite();
    let direct = Approval::Direct(
        prepare_direct_library_path(directory.path().as_os_str().to_owned())
            .expect("the staged suite makes the path ready"),
    )
    .record(&held);
    let later = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        None,
        None,
    )
    .expect("the recorded installation is still ready")
    .record(&held);
    let CheckPrepared::Direct {
        approval: direct_id,
        ..
    } = direct
    else {
        panic!("the first answer must approve the direct route");
    };

    assert!(
        matches!(held.consume(direct_id), Err(CheckRunError::Unapproved)),
        "a direct answer superseded by a bootstrap one must be refused"
    );
    assert!(
        matches!(later, CheckPrepared::Bootstrap { .. }),
        "the later answer must approve the bootstrap route"
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

/// Approve twice and show the run consumes only the newest answer.
///
/// This is the mutation `approve_readiness` names: a successful call holds a
/// run, and a second call replaces it. The earlier value is refused without
/// consuming the newer run, and the newer value then runs. The case exists to
/// prove the call is not read-only, which is why the port method states the
/// approval rather than asking a question.
#[cfg(unix)]
#[test]
fn the_newest_approval_is_the_only_runnable_one() {
    let (installation, held, first) = held_approval();
    let next = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        None,
        None,
    )
    .expect("the recorded installation is still ready")
    .record(&held);
    let CheckPrepared::Bootstrap {
        approval: second, ..
    } = next
    else {
        panic!("the second answer must approve the bootstrap route");
    };

    assert_ne!(
        first.number(),
        second.number(),
        "each answer must take its own approval number"
    );
    assert!(
        matches!(held.take(first), Err(CheckRunError::Unapproved)),
        "the superseded approval must not run"
    );
    assert!(
        held.take(second).is_ok(),
        "the newest approval must still hold its run"
    );
}

/// Refuse a number that was never issued.
///
/// The approval counter starts at zero, so a value naming zero — or anything
/// beyond the last answer — never came from this adapter. Accepting it would
/// let a hand-built value run without an inspection behind it.
#[cfg(unix)]
#[test]
fn an_approval_that_was_never_issued_is_refused() {
    let (_installation, held, current) = held_approval();
    let never_issued = ApprovalId::new(current.number() + 1);
    let zero = ApprovalId::new(0);

    assert!(
        matches!(held.consume(never_issued), Err(CheckRunError::Unapproved)),
        "a number beyond the last answer was never issued"
    );
    assert!(
        matches!(held.consume(zero), Err(CheckRunError::Unapproved)),
        "zero is the unissued starting point, not an approval"
    );
    assert!(
        held.take(current).is_ok(),
        "refusing the unissued numbers must not spend the current approval"
    );
}

/// Refuse a number that has already been consumed.
///
/// Consumption is what makes a value single-use. Without the recorded state, a
/// copied value would run a second time against an installation nobody
/// re-approved.
#[cfg(unix)]
#[test]
fn a_consumed_approval_cannot_be_spent_twice() {
    let (_installation, held, current) = held_approval();

    assert!(
        held.consume(current).is_ok(),
        "the current approval must be consumable once"
    );
    assert!(
        matches!(held.consume(current), Err(CheckRunError::Unapproved)),
        "a consumed approval must not be spendable again"
    );
    assert!(
        matches!(held.take(current), Err(CheckRunError::Unapproved)),
        "a consumed bootstrap approval must not yield its run again"
    );
}

/// Refuse a second run of the same direct value through `check_run`.
///
/// The direct route has no held run, so the only record that the value was
/// spent is the approval number. A second `check_run` with the same value must
/// be refused before Dylint starts rather than linting again unapproved.
#[cfg(unix)]
#[test]
fn a_direct_value_runs_once_through_check_run() {
    let directory = directory_with_suite();
    let path = prepare_direct_library_path(directory.path().as_os_str().to_owned())
        .expect("the staged suite makes the path ready");
    let services = SystemServices::new();
    let prepared = Approval::Direct(path).record(&services.approved);
    let CheckPrepared::Direct { .. } = &prepared else {
        panic!("a direct path must approve the direct route");
    };

    // The first run consumes the approval; the suite is then removed so a
    // second run would fail with a preflight refusal if it were allowed to
    // reach the re-check at all.
    services
        .check_run(prepared.clone(), &[], &mut Vec::new())
        .expect("the first run must be allowed");
    std::fs::remove_file(directory.path().join(suite_filename()))
        .expect("the approved suite can be removed");

    let second = services.check_run(prepared, &[], &mut Vec::new());

    assert!(
        matches!(second, Err(CheckRunError::Unapproved)),
        "a direct value must not run twice, got {second:?}"
    );
}

/// Keep a newer bootstrap approval runnable after a stale direct refusal.
///
/// A direct answer supersedes an earlier bootstrap one, and the bootstrap
/// answer that follows supersedes the direct one. Refusing the stale direct
/// value must leave the newer bootstrap approval's run untouched.
#[cfg(unix)]
#[test]
fn a_stale_direct_refusal_keeps_the_newer_bootstrap_approval() {
    let (installation, held, _first) = held_approval();
    let directory = directory_with_suite();
    let direct = Approval::Direct(
        prepare_direct_library_path(directory.path().as_os_str().to_owned())
            .expect("the staged suite makes the path ready"),
    )
    .record(&held);
    let CheckPrepared::Direct {
        approval: stale, ..
    } = direct
    else {
        panic!("the second answer must approve the direct route");
    };
    let newer = preflight_recorded_with_cargo(
        &installation.dirs,
        installation.cargo.as_os_str(),
        None,
        None,
    )
    .expect("the recorded installation is still ready")
    .record(&held);
    let CheckPrepared::Bootstrap {
        approval: current, ..
    } = newer
    else {
        panic!("the third answer must approve the bootstrap route");
    };

    assert!(
        matches!(held.consume(stale), Err(CheckRunError::Unapproved)),
        "the superseded direct approval must be refused"
    );
    assert!(
        held.take(current).is_ok(),
        "refusing the stale direct approval must leave the newer run available"
    );
}
