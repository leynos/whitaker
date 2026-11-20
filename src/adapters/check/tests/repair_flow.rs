//! The decisions a locked repair makes, driven through its two-step seam.
//!
//! Each test fixes the readiness answer and the install outcome, then asserts
//! the observable consequence: whether the installer ran, what it was asked
//! for, which stream each notice reached, and which error the command reports.
//! The seam exists so these paths run without a real installation.

use std::io::{self, Write};

use whitaker_installer::artefact::suite_ref::SuiteRef;

use super::{
    super::repair::{
        CI_NOTICE,
        REPAIR_NOTICE,
        Readiness,
        RepairRequest,
        RepairStreams,
        repair_with,
    },
    fixtures::{ScriptedSteps, record, temp_dirs},
};
use crate::{diagnostics::DiagnosticSettings, ports::CheckRepairError};

/// A stream that refuses every write, standing in for a closed descriptor.
struct ClosedStream;

impl Write for ClosedStream {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("the stream is closed"))
    }

    fn flush(&mut self) -> io::Result<()> { Err(io::Error::other("the stream is closed")) }
}

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
