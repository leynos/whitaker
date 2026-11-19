//! What a repair is asked for, and what its outcome reports.

use std::io;

use ortho_config::NoOpLocalizer;

use super::{ScriptedPort, arguments, dylint_version, not_installed};
use crate::{
    cli::check::{
        CheckRequest,
        Outcome,
        classify::RepairableReason,
        exit::CheckOutcome,
        run_check_command,
    },
    diagnostics::DiagnosticSettings,
    ports::CheckRepairError,
};

/// Repair a repairable installation once, then lint.
#[test]
fn a_repairable_installation_is_repaired_once_and_then_linted() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]);

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "a converged repair must reach Dylint's own status"
    );
    assert_eq!(port.repairs(), 1, "a repairable installation is repaired");
    assert_eq!(port.runs(), 1, "Dylint runs once, after the repair");
}

/// Never repair an installation that is already ready.
#[test]
fn a_ready_installation_is_never_repaired() {
    let port = ScriptedPort::answering(Ok(()));

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "a ready installation must reach Dylint's own status"
    );
    assert_eq!(
        port.repairs(),
        0,
        "a ready installation must not be provisioned again"
    );
}

/// Report a failed repair under its own key.
#[test]
fn a_failed_repair_is_reported_under_its_own_key() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]).failing_repair(
        CheckRepairError::Install {
            message: "the network is unavailable".to_owned(),
        },
    );
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut stderr,
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::RepairFailed),
        "an installer that ran and failed is a repair failure"
    );
    assert_eq!(port.runs(), 0, "Dylint must not run after a failed repair");
    let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
    assert!(
        rendered.contains("whitaker: error[provision.repair-failed]:"),
        "a failed repair must print its own key, got {rendered:?}"
    );
}

/// Report a repair that leaves the installation broken.
#[test]
fn a_repair_that_does_not_converge_is_reported() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Err(dylint_version())]);

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::NotConverged(RepairableReason::DylintVersion)),
        "a repair that leaves the installation broken is not a lint failure"
    );
    assert_eq!(
        port.runs(),
        0,
        "Dylint must not run against a stale install"
    );
}

/// Classify a lock failure as the host's fault rather than the installer's.
#[test]
fn a_lock_failure_is_a_host_fault_rather_than_an_install_failure() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]).failing_repair(
        CheckRepairError::Lock {
            message: "held by another process".to_owned(),
        },
    );

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::HostFailed(
            crate::cli::check::classify::HostFault::Lock
        )),
        "a lock that cannot be taken is the host's fault, not the installer's"
    );
}

/// Collect a repair through the command's own telemetry.
#[test]
fn a_repair_reaches_the_recorder_through_the_command() {
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]);

    metrics::with_local_recorder(&recorder, || {
        let outcome = run_check_command(
            CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
            &NoOpLocalizer::new(),
            &port,
            &mut io::sink(),
        );
        assert_eq!(
            outcome,
            Outcome::Check(CheckOutcome::Linted(0)),
            "the recorded invocation must converge and lint"
        );
    });

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        captured.iter().any(|(key, _, _, value)| {
            key.key().name() == crate::adapters::check::telemetry::CHECK_REPAIRS
                && key
                    .key()
                    .labels()
                    .any(|entry| entry.key() == "outcome" && entry.value() == "ok")
                && matches!(value, metrics_util::debugging::DebugValue::Counter(1))
        }),
        "a successful repair must produce a non-zero counter: {captured:#?}"
    );
}
