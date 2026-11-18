//! Wiring tests: the port is driven, and the flow's rules reach it.
//!
//! The repair policy is two environment facts, so these tests fix each
//! combination and assert the observable consequence: whether the repair port
//! was called, whether Dylint ran, and which outcome the command reports.

use std::{
    ffi::OsString,
    io::{self, Write},
    sync::Mutex,
};

use super::{
    CheckArgs,
    CheckOutcome,
    CheckServices,
    Outcome,
    classify::RepairableReason,
    run_check_command,
};
use crate::{
    diagnostics::DiagnosticSettings,
    ports::{CheckPreflightError, CheckPrepared, CheckRepairError, CheckRunError},
};

/// A port that answers from a script and records what it was asked.
struct ScriptedPort {
    /// The readiness answers, consumed in order; the last one repeats.
    answers: Mutex<Vec<Result<(), CheckPreflightError>>>,
    /// Whether the environment hands Dylint's libraries to the user.
    user_managed: bool,
    /// The failure a repair reports, if any.
    repair_failure: Mutex<Option<CheckRepairError>>,
    /// How many repairs were requested.
    repairs: Mutex<usize>,
    /// The status a run reports.
    status: i32,
    /// The arguments each run received.
    executed: Mutex<Vec<Vec<OsString>>>,
}

impl ScriptedPort {
    /// A port whose readiness answers are consumed in order.
    fn scripted(answers: Vec<Result<(), CheckPreflightError>>) -> Self {
        Self {
            answers: Mutex::new(answers),
            user_managed: false,
            repair_failure: Mutex::new(None),
            repairs: Mutex::new(0),
            status: 0,
            executed: Mutex::new(Vec::new()),
        }
    }

    /// A port that answers `answer` every time.
    fn answering(answer: Result<(), CheckPreflightError>) -> Self { Self::scripted(vec![answer]) }

    /// Settle whether the environment hands Dylint's libraries to the user.
    fn with_user_managed(mut self) -> Self {
        self.user_managed = true;
        self
    }

    /// Make every repair attempt fail with `error`.
    fn failing_repair(self, error: CheckRepairError) -> Self {
        *self.repair_failure.lock().expect("the test owns this lock") = Some(error);
        self
    }

    /// How many repairs the port was asked for.
    fn repairs(&self) -> usize { *self.repairs.lock().expect("the test owns this lock") }

    /// How many times Dylint was run.
    fn runs(&self) -> usize { self.executed.lock().expect("the test owns this lock").len() }
}

impl CheckServices for ScriptedPort {
    fn preflight(&self) -> Result<CheckPrepared, CheckPreflightError> {
        let mut answers = self.answers.lock().expect("the test owns this lock");
        let answer = if answers.len() > 1 {
            answers.remove(0)
        } else {
            answers.first().cloned().unwrap_or(Ok(()))
        };
        answer.map(|()| CheckPrepared::Direct {
            library_path: OsString::from("/lints"),
        })
    }

    fn library_path_is_user_managed(&self) -> bool { self.user_managed }

    fn repair(
        &self,
        _settings: DiagnosticSettings,
        _stderr: &mut dyn Write,
    ) -> Result<(), CheckRepairError> {
        *self.repairs.lock().expect("the test owns this lock") += 1;
        self.repair_failure
            .lock()
            .expect("the test owns this lock")
            .take()
            .map_or(Ok(()), Err)
    }

    fn check_run(
        &self,
        _prepared: CheckPrepared,
        args: &[OsString],
        _stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError> {
        self.executed
            .lock()
            .expect("the test owns this lock")
            .push(args.to_vec());
        Ok(self.status)
    }
}

/// Arguments for a default invocation, which may repair.
fn arguments(cargo_args: &[&str]) -> CheckArgs {
    CheckArgs {
        no_install: false,
        cargo_args: cargo_args.iter().map(OsString::from).collect(),
    }
}

/// Arguments for an invocation that refuses to change the machine.
fn refusing_arguments() -> CheckArgs {
    CheckArgs {
        no_install: true,
        cargo_args: Vec::new(),
    }
}

/// The bootstrap error for an installation that was never made.
fn not_installed() -> CheckPreflightError {
    CheckPreflightError::Bootstrap(
        whitaker_installer::bootstrap::BootstrapError::NotInstalled {
            path: "bootstrap.json".to_owned(),
        },
    )
}

/// The bootstrap error for a stale installed `cargo-dylint`.
fn dylint_version() -> CheckPreflightError {
    CheckPreflightError::Bootstrap(
        whitaker_installer::bootstrap::BootstrapError::DylintVersion {
            expected: "6.3.0".to_owned(),
            found: "6.2.0".to_owned(),
        },
    )
}

#[test]
fn a_ready_installation_reaches_the_port_with_whitakers_own_arguments() {
    let port = ScriptedPort::answering(Ok(()));

    let outcome = run_check_command(
        &arguments(&["-p", "x"]),
        DiagnosticSettings::default(),
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

#[test]
fn a_missing_installation_is_refused_before_any_run() {
    let port = ScriptedPort::answering(Err(not_installed()));
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        &refusing_arguments(),
        DiagnosticSettings::default(),
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

#[test]
fn a_user_managed_path_is_refused_with_its_own_key() {
    let port =
        ScriptedPort::answering(Err(CheckPreflightError::UserPathHasNoSuite)).with_user_managed();
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        &arguments(&[]),
        DiagnosticSettings::default(),
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

#[test]
fn a_host_fault_is_reported_without_advice() {
    let port = ScriptedPort::answering(Err(CheckPreflightError::Bootstrap(
        whitaker_installer::bootstrap::BootstrapError::NoDataDirectory,
    )));
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        &arguments(&[]),
        DiagnosticSettings::default(),
        &port,
        &mut stderr,
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::HostFailed(super::HostFault::DataDirectory)),
        "an undeterminable data directory is a host fault"
    );
    let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
    assert!(
        !rendered.contains("help:"),
        "installing again cannot mend a host fault, so no advice is offered: {rendered:?}"
    );
}

#[test]
fn a_repairable_installation_is_repaired_once_and_then_linted() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]);

    let outcome = run_check_command(
        &arguments(&[]),
        DiagnosticSettings::default(),
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

#[test]
fn a_ready_installation_is_never_repaired() {
    let port = ScriptedPort::answering(Ok(()));

    let outcome = run_check_command(
        &arguments(&[]),
        DiagnosticSettings::default(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(outcome, Outcome::Check(CheckOutcome::Linted(0)));
    assert_eq!(
        port.repairs(),
        0,
        "a ready installation must not be provisioned again"
    );
}

#[test]
fn no_install_forbids_repair() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]);

    let outcome = run_check_command(
        &refusing_arguments(),
        DiagnosticSettings::default(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Refused(RepairableReason::NotInstalled))
    );
    assert_eq!(port.repairs(), 0, "`--no-install` must win over repair");
}

#[test]
fn a_user_managed_path_is_never_repaired() {
    let port = ScriptedPort::answering(Err(dylint_version())).with_user_managed();

    let outcome = run_check_command(
        &arguments(&[]),
        DiagnosticSettings::default(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::Refused(RepairableReason::DylintVersion))
    );
    assert_eq!(
        port.repairs(),
        0,
        "installing Whitaker cannot change a path the user manages"
    );
}

#[test]
fn a_failed_repair_is_reported_under_its_own_key() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]).failing_repair(
        CheckRepairError::Install {
            message: "the network is unavailable".to_owned(),
        },
    );
    let mut stderr = Vec::new();

    let outcome = run_check_command(
        &arguments(&[]),
        DiagnosticSettings::default(),
        &port,
        &mut stderr,
    );

    assert_eq!(outcome, Outcome::Check(CheckOutcome::RepairFailed));
    assert_eq!(port.runs(), 0, "Dylint must not run after a failed repair");
    let rendered = String::from_utf8(stderr).expect("the diagnostic is UTF-8");
    assert!(
        rendered.contains("whitaker: error[provision.repair-failed]:"),
        "a failed repair must print its own key, got {rendered:?}"
    );
}

#[test]
fn a_repair_that_does_not_converge_is_reported() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Err(dylint_version())]);

    let outcome = run_check_command(
        &arguments(&[]),
        DiagnosticSettings::default(),
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

#[test]
fn a_lock_failure_is_a_host_fault_rather_than_an_install_failure() {
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]).failing_repair(
        CheckRepairError::Lock {
            message: "held by another process".to_owned(),
        },
    );

    let outcome = run_check_command(
        &arguments(&[]),
        DiagnosticSettings::default(),
        &port,
        &mut io::sink(),
    );

    assert_eq!(
        outcome,
        Outcome::Check(CheckOutcome::HostFailed(super::HostFault::Lock)),
        "a lock that cannot be taken is the host's fault, not the installer's"
    );
}

#[test]
fn a_repair_reaches_the_recorder_through_the_command() {
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let port = ScriptedPort::scripted(vec![Err(not_installed()), Ok(())]);

    metrics::with_local_recorder(&recorder, || {
        let outcome = run_check_command(
            &arguments(&[]),
            DiagnosticSettings::default(),
            &port,
            &mut io::sink(),
        );
        assert_eq!(outcome, Outcome::Check(CheckOutcome::Linted(0)));
    });

    let captured = snapshotter.snapshot().into_vec();
    assert!(
        captured.iter().any(|(key, _, _, value)| {
            key.key().name() == super::telemetry::CHECK_REPAIRS
                && key
                    .key()
                    .labels()
                    .any(|entry| entry.key() == "outcome" && entry.value() == "ok")
                && matches!(value, metrics_util::debugging::DebugValue::Counter(1))
        }),
        "a successful repair must produce a non-zero counter: {captured:#?}"
    );
}

#[test]
fn every_repairable_reason_has_a_stable_label() {
    for reason in [
        RepairableReason::NotInstalled,
        RepairableReason::Metadata,
        RepairableReason::LibrariesMissing,
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
