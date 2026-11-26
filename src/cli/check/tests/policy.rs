//! Which flow the policy selects, and what the command reports for it.

use std::{ffi::OsString, io};

use ortho_config::NoOpLocalizer;

use super::{ScriptedPort, arguments, dylint_version, not_installed, refusing_arguments};
use crate::{
    cli::check::{CheckRequest, Outcome, exit::CheckOutcome, run_check_command},
    diagnostics::DiagnosticSettings,
    ports::{BootstrapFault, CheckPreflightError, CheckRunError, HostFault, RepairableReason},
};

/// Drive the port with Whitaker's own arguments before the user's.
#[test]
fn a_ready_installation_reaches_the_port_with_whitakers_own_arguments() {
    let port = ScriptedPort::answering(Ok(()));

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&["-p", "x"]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Linted(0)),
        "a clean run must report Dylint's own status"
    );
    assert_eq!(
        port.executed
            .lock()
            .expect("the test owns this lock")
            .as_slice(),
        [vec![
            OsString::from("--lib=whitaker_suite"),
            OsString::from("--no-metadata"),
            OsString::from("--"),
            OsString::from("-p"),
            OsString::from("x"),
        ]],
        "the port must receive Whitaker's arguments before the user's"
    );
}

/// Refuse a missing installation without running Dylint.
#[test]
fn a_missing_installation_is_refused_before_any_run() {
    let port = ScriptedPort::answering(Err(not_installed()));
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        CheckRequest::new(&refusing_arguments(), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut stderr,
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Refused(RepairableReason::NotInstalled)),
        "a repairable defect with repair forbidden must be refused"
    );
    assert_eq!(port.runs(), 0, "Dylint must not run after a refusal");
    let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
    assert!(
        rendered.contains("whitaker: error[provision.refused]:"),
        "the refusal must print its stable key, got {rendered:?}"
    );
    assert!(
        rendered.contains("help: run `whitaker install`"),
        "the refusal must advise provisioning, got {rendered:?}"
    );
}

/// Collect a refusal through the command's own telemetry.
///
/// The counter test in the adapter proves `record_refusal` works; this one
/// proves the command actually calls it, with the reason the user was told.
#[test]
fn a_refusal_reaches_the_recorder_through_the_command() {
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let port = ScriptedPort::answering(Err(not_installed()));

    metrics::with_local_recorder(&recorder, || {
        let outcome = run_check_command(
            CheckRequest::new(&refusing_arguments(), DiagnosticSettings::default()),
            &NoOpLocalizer::new(),
            &port,
            &mut io::sink(),
        );
        assert_eq!(
            outcome,
            Outcome::Check(CheckOutcome::Refused(RepairableReason::NotInstalled)),
            "the recorded invocation must be refused"
        );
    });

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        captured.iter().any(|(key, _, _, value)| {
            key.key().name() == crate::adapters::check::telemetry::CHECK_REFUSALS
                && key
                    .key()
                    .labels()
                    .any(|entry| entry.key() == "reason" && entry.value() == "not_installed")
                && matches!(value, metrics_util::debugging::DebugValue::Counter(1))
        }),
        "a refusal must produce a non-zero counter under its reason: {captured:#?}"
    );
}

/// Refuse a user-managed path under its own key.
#[test]
fn a_user_managed_path_is_refused_with_its_own_key() {
    let port =
        ScriptedPort::answering(Err(CheckPreflightError::UserPathHasNoSuite)).with_user_managed();
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut stderr,
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Refused(RepairableReason::UserPath)),
        "a user-managed path failure keeps its own reason"
    );
    let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
    assert!(
        rendered.contains("whitaker: error[provision.user-path]:"),
        "a user path failure must print its own key, got {rendered:?}"
    );
}

/// Refuse a run fault the flow may not repair, under its own key.
///
/// The cases differ only in the route's data: readiness approved the path
/// moments earlier, and the run's own re-check found it changed. Repair is
/// forbidden in both — by `--no-install` for the recorded installation, by
/// ownership for the user's path — so nothing was installed, and the defect is
/// refused under the key its route uses at readiness. A regression that sent
/// either case to `classified` would print `provision.not-converged`, which
/// names a repair that never ran.
#[rstest::rstest]
#[case::user_path(
    true,
    CheckRunError::Preflight(CheckPreflightError::UserPathHasNoSuite),
    CheckOutcome::Refused(RepairableReason::UserPath),
    "provision.user-path"
)]
#[case::recorded_no_install(
    false,
    CheckRunError::Preflight(CheckPreflightError::Bootstrap(
        BootstrapFault::LibrariesMissing {
            path: "lints".to_owned(),
        },
    )),
    CheckOutcome::Refused(RepairableReason::LibrariesMissing),
    "provision.refused"
)]
fn a_run_fault_the_flow_may_not_repair_reports_its_refusal_key(
    #[case] is_user_managed: bool,
    #[case] fault: CheckRunError,
    #[case] expected: CheckOutcome,
    #[case] key: &str,
) {
    let scripted = ScriptedPort::answering(Ok(())).failing_run(fault);
    let port = if is_user_managed {
        scripted.with_user_managed()
    } else {
        scripted
    };
    let request = if is_user_managed {
        arguments(&[])
    } else {
        refusing_arguments()
    };
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        CheckRequest::new(&request, DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut stderr,
    );

    assert_eq!(
        outcome,
        Outcome::Check(expected),
        "a run fault the flow may not repair must be refused under {key}"
    );
    assert_eq!(
        port.repairs(),
        0,
        "no repair may be spent on a refusal under {key}"
    );
    assert_eq!(
        port.runs(),
        1,
        "the run that found the defect is the one the refusal reports: {key}"
    );
    let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
    assert!(
        rendered.contains(&format!("whitaker: error[{key}]:")),
        "the refusal must print {key}, got {rendered:?}"
    );
    assert!(
        !rendered.contains("provision.not-converged"),
        "no repair ran, so there is nothing that failed to converge: {rendered:?}"
    );
}

/// Report a host fault without offering repair advice.
///
/// The cases cover both host classes the classifier can produce from a
/// bootstrap error, so the flow is exercised through the classifier rather
/// than by constructing the fault directly: changing how an unreadable
/// directory is classified must break this test.
#[rstest::rstest]
#[case::data_directory(BootstrapFault::NoDataDirectory, HostFault::DataDirectory)]
#[case::io(
    BootstrapFault::Io {
        path: "lints".to_owned(),
        reason: "denied".to_owned(),
    },
    HostFault::Io
)]
fn a_host_fault_is_reported_without_advice(
    #[case] error: BootstrapFault,
    #[case] expected: HostFault,
) {
    let port = ScriptedPort::answering(Err(CheckPreflightError::Bootstrap(error)));
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut stderr,
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::HostFailed(expected)),
        "a bootstrap host fault must reach the user as the same host fault"
    );
    let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
    assert!(
        !rendered.contains("help:"),
        "installing again cannot mend a host fault, so no advice is offered: {rendered:?}"
    );
}

/// Let `--no-install` win over repair.
#[test]
fn no_install_forbids_repair() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]);

    let outcome = run_check_command(
        CheckRequest::new(&refusing_arguments(), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Refused(RepairableReason::NotInstalled)),
        "a repairable defect with repair forbidden must be refused"
    );
    assert_eq!(port.repairs(), 0, "`--no-install` must win over repair");
}

/// Never repair a path the user manages.
#[test]
fn a_user_managed_path_is_never_repaired() {
    let port = ScriptedPort::answering(Err(dylint_version())).with_user_managed();

    let outcome = run_check_command(
        CheckRequest::new(&arguments(&[]), DiagnosticSettings::default()),
        &NoOpLocalizer::new(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Refused(RepairableReason::DylintVersion)),
        "a user-managed path keeps its own reason rather than being repaired"
    );
    assert_eq!(
        port.repairs(),
        0,
        "installing Whitaker cannot change a path the user manages"
    );
}

/// Label every repairable reason with a stable identifier.
#[test]
fn every_repairable_reason_has_a_stable_label() {
    for reason in [
        RepairableReason::NotInstalled,
        RepairableReason::Metadata,
        RepairableReason::LibrariesMissing,
        RepairableReason::LibrariesAmbiguous,
        RepairableReason::Snapshot,
        RepairableReason::CargoDylint,
        RepairableReason::DylintVersion,
        RepairableReason::UserPath,
    ] {
        let label = reason.label();
        assert!(
            !label.is_empty() && label.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "a metric label must be a stable identifier, got {label:?}"
        );
    }
}
