//! Wiring tests: the port is driven, and the flow's rules reach it.
//!
//! The repair policy is two environment facts, so these tests fix each
//! combination and assert the observable consequence: whether the repair port
//! was called, whether Dylint ran, and which outcome the command reports.

use std::{ffi::OsString, io::Write, sync::Mutex};

use camino::Utf8PathBuf;
use tempfile::TempDir;
use whitaker_installer::{crate_name::CrateName, resolution::SUITE_CRATE, stager::Stager};

use super::{CheckArgs, CheckServices};
use crate::{
    diagnostics::DiagnosticSettings,
    ports::{
        CheckPreflightError,
        CheckPrepared,
        CheckRepairError,
        CheckRunError,
        DirectLibraryPath,
    },
};

/// A staged Whitaker suite library, in a directory the port keeps alive.
///
/// A prepared value can only be obtained by inspection, so the fake needs a
/// real directory holding exactly one suite library rather than a placeholder
/// path: the port cannot otherwise report a successful readiness check.
struct SuiteLibrary {
    /// Kept alive for as long as the port hands out the prepared value.
    _directory: TempDir,
    /// The ready value a successful preflight reports.
    prepared: CheckPrepared,
}

impl SuiteLibrary {
    /// Stage one suite library in a fresh temporary directory.
    fn staged() -> Self {
        let directory = TempDir::new().expect("a temporary directory is available");
        let filename = Stager::new(Utf8PathBuf::from("unused"), "nightly-2026-05-28")
            .staged_filename(&CrateName::from(SUITE_CRATE));
        std::fs::write(directory.path().join(filename), b"")
            .expect("the suite file can be created");
        let prepared = DirectLibraryPath::new(directory.path().as_os_str().to_owned())
            .map(CheckPrepared::Direct)
            .expect("the staged suite makes the path ready");
        Self {
            _directory: directory,
            prepared,
        }
    }
}

/// A port that answers from a script and records what it was asked.
struct ScriptedPort {
    /// The readiness answers, consumed in order; the last one repeats.
    answers: Mutex<Vec<Result<(), CheckPreflightError>>>,
    /// The library path a successful answer reports.
    library: SuiteLibrary,
    /// Whether the environment hands Dylint's libraries to the user.
    user_managed: bool,
    /// The failure a repair reports, if any.
    repair_failure: Mutex<Option<CheckRepairError>>,
    /// The failure the next run reports, if any.
    run_failure: Mutex<Option<CheckRunError>>,
    /// How many repairs were requested.
    repairs: Mutex<usize>,
    /// The status a successful run reports.
    status: i32,
    /// The arguments each run received.
    executed: Mutex<Vec<Vec<OsString>>>,
}

impl ScriptedPort {
    /// A port whose readiness answers are consumed in order.
    fn scripted(answers: Vec<Result<(), CheckPreflightError>>) -> Self {
        Self {
            answers: Mutex::new(answers),
            library: SuiteLibrary::staged(),
            user_managed: false,
            repair_failure: Mutex::new(None),
            run_failure: Mutex::new(None),
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

    /// Make the next run fail with `error`, as a change after readiness does.
    fn failing_run(self, error: CheckRunError) -> Self {
        *self.run_failure.lock().expect("the test owns this lock") = Some(error);
        self
    }

    /// Count the repairs the port was asked for.
    fn repairs(&self) -> usize { *self.repairs.lock().expect("the test owns this lock") }

    /// Count the times Dylint was run.
    fn runs(&self) -> usize { self.executed.lock().expect("the test owns this lock").len() }
}

impl CheckServices for ScriptedPort {
    /// Consume the next scripted readiness answer, repeating the last one.
    fn preflight(&self) -> Result<CheckPrepared, CheckPreflightError> {
        let mut answers = self.answers.lock().expect("the test owns this lock");
        let answer = if answers.len() > 1 {
            answers.remove(0)
        } else {
            answers.first().cloned().unwrap_or(Ok(()))
        };
        answer.map(|()| self.library.prepared.clone())
    }

    /// Report the environment fact the test settled.
    fn library_path_is_user_managed(&self) -> bool { self.user_managed }

    /// Count the request and report the scripted failure, once.
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

    /// Record the arguments and report the scripted status or failure.
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
        self.run_failure
            .lock()
            .expect("the test owns this lock")
            .take()
            .map_or(Ok(self.status), Err)
    }
}

/// Build the arguments for a default invocation, which may repair.
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

/// Build the bootstrap error for an installation that was never made.
fn not_installed() -> CheckPreflightError {
    CheckPreflightError::Bootstrap(
        whitaker_installer::bootstrap::BootstrapError::NotInstalled {
            path: "bootstrap.json".to_owned(),
        },
    )
}

/// Build the bootstrap error for a stale installed `cargo-dylint`.
fn dylint_version() -> CheckPreflightError {
    CheckPreflightError::Bootstrap(
        whitaker_installer::bootstrap::BootstrapError::DylintVersion {
            expected: "6.3.0".to_owned(),
            found: "6.2.0".to_owned(),
        },
    )
}

mod faults;
mod policy;
mod repair;
mod telemetry;
