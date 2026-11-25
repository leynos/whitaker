//! Fake ports that script the lazy-repair scenarios.
//!
//! The scenarios drive the real router and command with these fakes, so the
//! decision under test is the one that ships: no `cargo`, no network and no
//! filesystem beyond the staged suite a prepared value has to prove. The fake
//! check records every call in one ordered trace, which is how a scenario can
//! require the readiness recheck to precede the Dylint run.

use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    io::Write,
};

use camino::Utf8PathBuf;
use tempfile::TempDir;
use whitaker::{
    adapters::check::prepare_direct_library_path,
    diagnostics::DiagnosticSettings,
    ports::{
        BootstrapFault,
        CheckPreflightError,
        CheckPrepared,
        CheckRepairError,
        CheckRunError,
        CheckServices,
        CliError,
        InstallRequest,
        InstallService,
        LintInventory,
        LintRunner,
        ListRequest,
        StagedInventory,
    },
};
use whitaker_installer::{crate_name::CrateName, resolution::SUITE_CRATE, stager::Stager};

/// The marker an installer writes to the standard output it is given.
pub(super) const MARKER: &str = "suite-source: whitelisted";

/// One scripted readiness answer, named so the trace can report it.
#[derive(Clone)]
pub(super) struct Answer {
    /// The feature file's name for this answer.
    pub(super) name: String,
    /// What the fake port returns for it.
    pub(super) result: Result<(), CheckPreflightError>,
}

/// Map a scenario's reason name onto the answer a readiness check gives.
///
/// The outer error names a typo in the feature file; the inner one is the
/// answer the fake port gives. The answers are the port's own faults, because
/// that is all the port can report: the installer's vocabulary stops at its
/// adapter.
pub(super) fn answer_for(reason: &str) -> Result<Answer, String> {
    let fault = match reason {
        "Ready" => {
            return Ok(Answer {
                name: reason.to_owned(),
                result: Ok(()),
            });
        }
        "NotInstalled" => BootstrapFault::NotInstalled {
            path: "bootstrap.json".to_owned(),
        },
        "LibrariesMissing" => BootstrapFault::LibrariesMissing {
            path: "lints".to_owned(),
        },
        "Snapshot" => BootstrapFault::Snapshot {
            reason: "the snapshot manifest is missing".to_owned(),
        },
        "DylintVersion" => BootstrapFault::DylintVersion {
            expected: "6.3.0".to_owned(),
            found: "6.2.0".to_owned(),
        },
        other => {
            return Err(format!(
                "the scenario names no readiness answer called {other}"
            ));
        }
    };
    Ok(Answer {
        name: reason.to_owned(),
        result: Err(CheckPreflightError::Bootstrap(fault)),
    })
}

/// A staged suite library, in a directory the fake keeps alive.
///
/// A prepared value can only be obtained by inspection, so the fake needs a
/// real directory holding exactly one suite library rather than a placeholder
/// path: it cannot otherwise report a successful readiness check.
pub(super) struct SuiteLibrary {
    /// Kept alive for as long as the fake hands out the prepared value.
    _directory: TempDir,
    /// The ready value a successful preflight reports.
    prepared: CheckPrepared,
}

impl SuiteLibrary {
    /// Stage one suite library in a fresh temporary directory.
    ///
    /// # Errors
    ///
    /// Returns the reason the library could not be staged, so the scenario
    /// reports a broken harness instead of panicking inside a step.
    pub(super) fn staged() -> Result<Self, String> {
        let directory = TempDir::new().map_err(|error| error.to_string())?;
        let filename = Stager::new(Utf8PathBuf::from("unused"), "nightly-2026-05-28")
            .staged_filename(&CrateName::from(SUITE_CRATE));
        std::fs::write(directory.path().join(filename), b"").map_err(|error| error.to_string())?;
        let prepared = prepare_direct_library_path(directory.path().as_os_str().to_owned())
            .map(CheckPrepared::Direct)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            _directory: directory,
            prepared,
        })
    }
}

/// The `check` port, scripted by the scenario.
pub(super) struct FakeCheck {
    /// The readiness answers, consumed in order.
    answers: RefCell<Vec<Answer>>,
    /// The library path a successful answer reports.
    library: SuiteLibrary,
    /// Whether a repair reports a failure.
    repair_fails: bool,
    /// Whether the library path belongs to the user.
    user_managed: bool,
    /// How many repairs were requested.
    repairs: Cell<usize>,
    /// How many Dylint runs were requested.
    runs: Cell<usize>,
    /// What the flow asked for, in call order, as the feature file names it.
    trace: RefCell<Vec<String>>,
}

impl FakeCheck {
    /// Build the port from one scenario's script.
    pub(super) const fn new(
        answers: Vec<Answer>,
        library: SuiteLibrary,
        repair_fails: bool,
        user_managed: bool,
    ) -> Self {
        Self {
            answers: RefCell::new(answers),
            library,
            repair_fails,
            user_managed,
            repairs: Cell::new(0),
            runs: Cell::new(0),
            trace: RefCell::new(Vec::new()),
        }
    }

    /// How many repairs the flow requested.
    pub(super) const fn repairs(&self) -> usize { self.repairs.get() }

    /// How many Dylint runs the flow requested.
    pub(super) const fn runs(&self) -> usize { self.runs.get() }

    /// The calls the flow made, in order, as the feature file names them.
    pub(super) fn trace(&self) -> Vec<String> { self.trace.borrow().clone() }
}

impl CheckServices for FakeCheck {
    /// Record the preflight answer and report it.
    fn approve_readiness(&self) -> Result<CheckPrepared, CheckPreflightError> {
        let mut answers = self.answers.borrow_mut();
        let answer = if answers.len() > 1 {
            answers.remove(0)
        } else {
            answers.first().cloned().unwrap_or_else(|| Answer {
                name: "Ready".to_owned(),
                result: Ok(()),
            })
        };
        self.trace.borrow_mut().push(answer.name.clone());
        answer.result.map(|()| self.library.prepared.clone())
    }

    /// Report whether the library path belongs to the user.
    fn library_path_is_user_managed(&self) -> bool { self.user_managed }

    /// Record the repair, write the marker, then report the scripted failure.
    ///
    /// The marker is written before the failure is returned, so a failed
    /// repair's captured output still shows what the installer wrote.
    fn repair(
        &self,
        _settings: DiagnosticSettings,
        stderr: &mut dyn Write,
    ) -> Result<(), CheckRepairError> {
        self.repairs.set(self.repairs.get() + 1);
        self.trace.borrow_mut().push("repair".to_owned());
        // The installer writes its machine-readable marker to the stream it is
        // given; `check` hands it standard error so the marker cannot pollute
        // a JSON pipeline.
        writeln!(stderr, "{MARKER}").map_err(|error| CheckRepairError::Host {
            message: error.to_string(),
        })?;
        if self.repair_fails {
            return Err(CheckRepairError::Install {
                message: "the install failed".to_owned(),
            });
        }
        Ok(())
    }

    /// Record the run and report a successful lint.
    fn check_run(
        &self,
        _prepared: CheckPrepared,
        _args: &[OsString],
        _stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError> {
        self.runs.set(self.runs.get() + 1);
        self.trace.borrow_mut().push("run".to_owned());
        Ok(0)
    }
}

/// An install port that records calls and reports success.
#[derive(Default)]
pub(super) struct FakeInstall {
    /// How many installs were requested.
    calls: Cell<usize>,
}

impl InstallService for FakeInstall {
    /// Record the install and report success.
    fn install(
        &self,
        _request: &InstallRequest,
        _stdout: &mut dyn Write,
        _stderr: &mut dyn Write,
    ) -> Result<(), CliError> {
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
}

/// An inventory port the check route never reaches.
pub(super) struct UnusedInventory;

impl LintInventory for UnusedInventory {
    /// Report that the check route does not list lints.
    fn list(&self, _request: &ListRequest) -> Result<StagedInventory, CliError> {
        Err(CliError::Inventory {
            message: "the check route does not list lints".to_owned(),
        })
    }
}

/// A forwarding port the check route never reaches.
pub(super) struct UnusedRunner;

impl LintRunner for UnusedRunner {
    /// Report that the check route does not forward.
    fn forward(&self, _args: &[OsString], _stderr: &mut dyn Write) -> Result<i32, CliError> {
        Err(CliError::Forward {
            source: std::io::Error::other("the check route does not forward"),
        })
    }
}
