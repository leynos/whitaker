//! The decisions a locked repair makes, driven through its two-step seam.
//!
//! Each test fixes the readiness answer and the install outcome, then asserts
//! the observable consequence: whether the installer ran, what it was asked
//! for, which stream each notice reached, and which error the command reports.
//! The seam exists so these paths run without a real installation.

use std::io::Write;

use whitaker_installer::artefact::suite_ref::SuiteRef;

#[cfg(unix)]
use super::fixtures::RealReadiness;
use super::{
    super::repair::{
        CI_NOTICE,
        REPAIR_NOTICE,
        Readiness,
        RepairRequest,
        RepairStreams,
        repair_with,
    },
    fixtures::{ClosedStream, ScriptedSteps, record, temp_dirs},
};
use crate::{diagnostics::DiagnosticSettings, ports::CheckRepairError};

/// Run one scripted repair, returning the outcome and every buffer it wrote.
fn run_repair(
    steps: &mut ScriptedSteps,
    is_ci: bool,
    diagnostics: &mut dyn Write,
) -> (Result<(), CheckRepairError>, Vec<u8>) {
    let (root, dirs) = temp_dirs();
    record(None)
        .write(&dirs.layout().metadata_path())
        .expect("the record can be written");
    let mut installer = Vec::new();
    let outcome = repair_with(
        &dirs,
        RepairRequest {
            settings: DiagnosticSettings::default(),
            is_ci,
        },
        RepairStreams {
            installer: &mut installer,
            diagnostics,
        },
        steps,
    );
    drop(root);
    (outcome, installer)
}

/// Do nothing, and say nothing, when the locked check finds a ready machine.
#[test]
fn a_ready_installation_is_not_installed_again() {
    let mut steps = ScriptedSteps::ready_to_install(Readiness::Ready);
    let mut diagnostics = Vec::new();

    let (outcome, _installer) = run_repair(&mut steps, false, &mut diagnostics);

    assert!(
        outcome.is_ok(),
        "an installation that became ready under the lock needs no work, got {outcome:?}"
    );
    assert_eq!(
        steps.installs, 0,
        "a ready installation is never reinstalled"
    );
    assert!(
        diagnostics.is_empty(),
        "a repair that does nothing must say nothing: {}",
        String::from_utf8_lossy(&diagnostics)
    );
}

/// Report a host fault, keeping its message, without attempting an install.
#[test]
fn a_host_fault_is_reported_without_installing() {
    let mut steps = ScriptedSteps::ready_to_install(Readiness::Host {
        message: "the data directory is unreadable".to_owned(),
    });
    let mut diagnostics = Vec::new();

    let (outcome, _installer) = run_repair(&mut steps, false, &mut diagnostics);

    assert!(
        matches!(
            &outcome,
            Err(CheckRepairError::Host { message })
                if message == "the data directory is unreadable"
        ),
        "a host fault must survive the repair with its own message, got {outcome:?}"
    );
    assert_eq!(
        steps.installs, 0,
        "installing cannot mend a host fault, so it must not be attempted"
    );
}

/// Install once, with the recorded pin, announcing the repair but not CI.
#[test]
fn a_repairable_installation_is_installed_once_with_the_recorded_pin() {
    let (root, dirs) = temp_dirs();
    record(Some("v0.2.7"))
        .write(&dirs.layout().metadata_path())
        .expect("the record can be written");
    let mut steps = ScriptedSteps::ready_to_install(Readiness::Repairable);
    let mut diagnostics = Vec::new();
    let mut installer = Vec::new();

    let outcome = repair_with(
        &dirs,
        RepairRequest {
            settings: DiagnosticSettings::default(),
            is_ci: false,
        },
        RepairStreams {
            installer: &mut installer,
            diagnostics: &mut diagnostics,
        },
        &mut steps,
    );
    drop(root);

    assert!(
        outcome.is_ok(),
        "a completed repair reports success, got {outcome:?}"
    );
    assert_eq!(
        steps.installs, 1,
        "a repairable installation is repaired once"
    );
    let recorded_pin = steps
        .arguments
        .first()
        .and_then(|arguments| arguments.suite_version.as_ref())
        .map(SuiteRef::as_str);
    assert_eq!(
        recorded_pin,
        Some("v0.2.7"),
        "the repair must ask for the suite the installation recorded"
    );
    let written = String::from_utf8(diagnostics).expect("the notices are UTF-8");
    assert!(
        written.lines().any(|line| line == REPAIR_NOTICE),
        "the repair must announce itself: {written}"
    );
    assert!(
        !written.contains(CI_NOTICE),
        "a repair outside CI must not give CI advice: {written}"
    );
}

/// Add the CI-route advice, without changing the outcome, when running in CI.
#[test]
fn a_repair_in_ci_also_advises_the_ci_route() {
    let mut steps = ScriptedSteps::ready_to_install(Readiness::Repairable);
    let mut diagnostics = Vec::new();

    let (outcome, _installer) = run_repair(&mut steps, true, &mut diagnostics);

    assert!(
        outcome.is_ok(),
        "the CI advice does not change the outcome, got {outcome:?}"
    );
    let written = String::from_utf8(diagnostics).expect("the notices are UTF-8");
    assert!(
        written.lines().any(|line| line == REPAIR_NOTICE),
        "every repair announces itself: {written}"
    );
    assert!(
        written.lines().any(|line| line == CI_NOTICE),
        "a repair in CI must explain the cheaper route: {written}"
    );
}

/// Announce the repair before an install that then fails.
#[test]
fn a_failed_install_is_reported_after_the_notice() {
    let mut steps = ScriptedSteps::failing("whitaker-installer: installing suite");
    let mut diagnostics = Vec::new();

    let (outcome, _installer) = run_repair(&mut steps, false, &mut diagnostics);

    assert!(
        matches!(outcome, Err(CheckRepairError::Install { .. })),
        "an installer failure is the installer's to report, got {outcome:?}"
    );
    let written = String::from_utf8(diagnostics).expect("the notices are UTF-8");
    assert!(
        written.lines().any(|line| line == REPAIR_NOTICE),
        "the notice must precede an install that then fails: {written}"
    );
}

/// Keep the installer's machine markers off the human diagnostics stream.
#[test]
fn the_installers_markers_stay_off_the_diagnostics_stream() {
    let mut steps = ScriptedSteps::ready_to_install(Readiness::Repairable)
        .writing("whitaker-installer: staged");
    let mut diagnostics = Vec::new();

    let (outcome, installer) = run_repair(&mut steps, false, &mut diagnostics);

    assert!(
        outcome.is_ok(),
        "the marker does not change the outcome, got {outcome:?}"
    );
    let markers = String::from_utf8(installer).expect("the markers are UTF-8");
    assert_eq!(
        markers.lines().collect::<Vec<_>>(),
        vec!["whitaker-installer: staged"],
        "the installer's markers must reach the stream reserved for them"
    );
    let written = String::from_utf8(diagnostics).expect("the notices are UTF-8");
    assert!(
        !written.contains("whitaker-installer: staged"),
        "a machine marker must not be mixed into the human diagnostics: {written}"
    );
}

/// Refuse a locked preflight host fault, leaving the installer uncalled.
///
/// The case is a machine whose installation gap was repairable when readiness
/// looked, but whose data directory became unreadable by the time the lock was
/// held. The re-check then fails with an I/O error, which installing cannot
/// mend: attempting it would report "repair failed" and hide the real cause.
/// Readiness is the production check here, so the test measures the policy
/// rather than a scripted restatement of it.
///
/// Unix-only because the provocation is: a file standing where the data
/// directory belongs is `ENOTDIR` here, which is a filesystem fault, but
/// Windows reports the same path as not found, the answer a missing record
/// also gives. The policy itself is covered on every host by
/// `a_host_fault_is_reported_without_installing`, which scripts the in-lock
/// answer instead of provoking it.
#[cfg(unix)]
#[test]
fn a_locked_preflight_host_fault_is_reported_without_installing() {
    let (root, dirs) = temp_dirs();
    // A regular file where the data directory belongs makes every read beneath
    // it fail with ENOTDIR — a filesystem fault, not a missing record.
    std::fs::write(root.path().join("data"), b"not a directory")
        .expect("a file can stand where the data directory belongs");
    let mut steps = RealReadiness::new();
    let mut diagnostics = Vec::new();
    let mut installer = Vec::new();

    let outcome = repair_with(
        &dirs,
        RepairRequest {
            settings: DiagnosticSettings::default(),
            is_ci: false,
        },
        RepairStreams {
            installer: &mut installer,
            diagnostics: &mut diagnostics,
        },
        &mut steps,
    );

    assert!(
        matches!(outcome, Err(CheckRepairError::Host { .. })),
        "an unreadable data directory is the host's fault, got {outcome:?}"
    );
    assert_eq!(
        steps.installs, 0,
        "installing cannot mend a host fault, so it must not be attempted"
    );
    assert!(
        diagnostics.is_empty(),
        "a repair that never starts must not announce itself: {}",
        String::from_utf8_lossy(&diagnostics)
    );
}

/// Stop before installing when the repair notice cannot be written.
#[test]
fn an_unwritable_notice_stops_the_repair_before_installing() {
    let mut steps = ScriptedSteps::ready_to_install(Readiness::Repairable);
    let mut closed = ClosedStream;

    let (outcome, _installer) = run_repair(&mut steps, false, &mut closed);

    assert!(
        matches!(outcome, Err(CheckRepairError::Output { .. })),
        "a stream that refuses the notice is an output failure, got {outcome:?}"
    );
    assert_eq!(
        steps.installs, 0,
        "an install whose start cannot be announced must not run"
    );
}
