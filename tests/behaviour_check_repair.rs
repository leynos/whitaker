//! In-process scenarios for lazy repair in `whitaker check`.
//!
//! The scenarios drive the real router and command with fake ports, so the
//! decision under test is the one that ships: no `cargo`, no network and no
//! filesystem. The fake repair writes the installer's `suite-source` marker
//! into the stream it is handed, which is standard error, and the captured
//! standard output must stay empty — during `check`, every byte on standard
//! output has to come from the Dylint child.

use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    io::Write,
};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker::{
    cli::{self, CommandStreams, exit::exit_code_for},
    diagnostics::DiagnosticSettings,
    ports::{
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
        Services,
        StagedInventory,
    },
};
use whitaker_installer::{bootstrap::BootstrapError, clippy_snapshot::ClippySnapshotError};

/// The marker an installer writes to the standard output it is given.
const MARKER: &str = "suite-source: whitelisted";

/// The state one scenario accumulates.
#[derive(Default)]
struct RepairWorld {
    /// What the readiness check answers, in order; the last answer repeats.
    answers: RefCell<Vec<Result<(), CheckPreflightError>>>,
    /// Whether the repair fails.
    repair_fails: RefCell<bool>,
    /// Whether the environment hands the library path to the user.
    user_managed: RefCell<bool>,
    /// What the invocation produced.
    recorded: RefCell<Option<Invocation>>,
}

/// Everything a scenario asserts on after an invocation.
struct Invocation {
    /// The outcome the router reported.
    outcome: cli::exit::Outcome,
    /// Everything Whitaker wrote to standard output.
    stdout: String,
    /// Everything Whitaker wrote to standard error.
    stderr: String,
    /// How many repairs the check port was asked for.
    repairs: usize,
    /// How many Dylint runs the check port was asked for.
    runs: usize,
}

impl Invocation {
    /// The process exit code the outcome maps to.
    const fn code(&self) -> i32 { exit_code_for(&self.outcome) }
}

impl RepairWorld {
    /// The recorded invocation, if the scenario ran one.
    fn invocation(&self) -> Result<std::cell::Ref<'_, Invocation>, String> {
        std::cell::Ref::filter_map(self.recorded.borrow(), Option::as_ref)
            .map_err(|_| "the check invocation should be recorded".to_owned())
    }

    /// Run `whitaker check` with `arguments` through the real router.
    fn run(&self, arguments: &[&str]) -> Result<(), String> {
        let repair_fails = *self.repair_fails.borrow();
        let user_managed = *self.user_managed.borrow();
        let check = FakeCheck {
            answers: RefCell::new(self.answers.borrow().clone()),
            repair_fails,
            user_managed,
            repairs: Cell::new(0),
            runs: Cell::new(0),
        };
        let install = FakeInstall::default();
        let services = Services {
            install: &install,
            inventory: &UnusedInventory,
            lint_runner: &UnusedRunner,
            check: &check,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut invocation: Vec<OsString> = vec!["whitaker".into(), "check".into()];
        invocation.extend(arguments.iter().copied().map(OsString::from));
        let outcome = cli::run_from(
            invocation,
            &services,
            &ortho_config::NoOpLocalizer::new(),
            &mut CommandStreams::new(&mut stdout, &mut stderr),
        );
        self.recorded.replace(Some(Invocation {
            outcome,
            stdout: String::from_utf8(stdout).map_err(|_| "stdout should be UTF-8")?,
            stderr: String::from_utf8(stderr).map_err(|_| "stderr should be UTF-8")?,
            repairs: check.repairs.get(),
            runs: check.runs.get(),
        }));
        Ok(())
    }
}

/// The `check` port, scripted by the scenario.
struct FakeCheck {
    /// The readiness answers, consumed in order.
    answers: RefCell<Vec<Result<(), CheckPreflightError>>>,
    /// Whether a repair reports a failure.
    repair_fails: bool,
    /// Whether the library path belongs to the user.
    user_managed: bool,
    /// How many repairs were requested.
    repairs: Cell<usize>,
    /// How many Dylint runs were requested.
    runs: Cell<usize>,
}

impl CheckServices for FakeCheck {
    fn preflight(&self) -> Result<CheckPrepared, CheckPreflightError> {
        let mut answers = self.answers.borrow_mut();
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
        stderr: &mut dyn Write,
    ) -> Result<(), CheckRepairError> {
        self.repairs.set(self.repairs.get() + 1);
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

    fn check_run(
        &self,
        _prepared: CheckPrepared,
        _args: &[OsString],
        _stderr: &mut dyn Write,
    ) -> Result<i32, CheckRunError> {
        self.runs.set(self.runs.get() + 1);
        Ok(0)
    }
}

/// An install port that records calls and reports success.
#[derive(Default)]
struct FakeInstall {
    /// How many installs were requested.
    calls: Cell<usize>,
}

impl InstallService for FakeInstall {
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
struct UnusedInventory;

impl LintInventory for UnusedInventory {
    fn list(&self, _request: &ListRequest) -> Result<StagedInventory, CliError> {
        Err(CliError::Inventory {
            message: "the check route does not list lints".to_owned(),
        })
    }
}

/// A forwarding port the check route never reaches.
struct UnusedRunner;

impl LintRunner for UnusedRunner {
    fn forward(&self, _args: &[OsString], _stderr: &mut dyn Write) -> Result<i32, CliError> {
        Err(CliError::Forward {
            source: std::io::Error::other("the check route does not forward"),
        })
    }
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> RepairWorld { RepairWorld::default() }

/// Map a scenario's reason name onto the answer a readiness check gives.
///
/// The outer error names a typo in the feature file; the inner one is the
/// answer the fake port gives.
fn answer_for(reason: &str) -> Result<Result<(), CheckPreflightError>, String> {
    let error = match reason {
        "Ready" => return Ok(Ok(())),
        "NotInstalled" => BootstrapError::NotInstalled {
            path: "bootstrap.json".to_owned(),
        },
        "LibrariesMissing" => BootstrapError::LibrariesMissing {
            path: "lints".to_owned(),
        },
        "Snapshot" => BootstrapError::Snapshot(ClippySnapshotError::Manifest {
            reason: "missing".to_owned(),
        }),
        "DylintVersion" => BootstrapError::DylintVersion {
            expected: "6.3.0".to_owned(),
            found: "6.2.0".to_owned(),
        },
        other => {
            return Err(format!(
                "the scenario names no readiness answer called {other}"
            ));
        }
    };
    Ok(Err(CheckPreflightError::Bootstrap(error)))
}

#[given("preflight reports \"{reason}\"")]
fn given_preflight_reports(world: &RepairWorld, reason: &str) -> Result<(), String> {
    world.answers.borrow_mut().push(answer_for(reason)?);
    Ok(())
}

#[given("preflight reports \"{first}\" and then \"{second}\"")]
fn given_preflight_reports_twice(
    world: &RepairWorld,
    first: &str,
    second: &str,
) -> Result<(), String> {
    world.answers.borrow_mut().push(answer_for(first)?);
    world.answers.borrow_mut().push(answer_for(second)?);
    Ok(())
}

#[given("the repair succeeds")]
fn given_repair_succeeds(world: &RepairWorld) { world.repair_fails.replace(false); }

#[given("the repair fails")]
fn given_repair_fails(world: &RepairWorld) { world.repair_fails.replace(true); }

#[given("a user-managed library path")]
fn given_user_managed_path(world: &RepairWorld) { world.user_managed.replace(true); }

#[when("check runs with default options")]
fn when_check_runs(world: &RepairWorld) -> Result<(), String> { world.run(&[] as &[&str]) }

#[when("check runs with \"{arguments}\"")]
fn when_check_runs_with(world: &RepairWorld, arguments: &str) -> Result<(), String> {
    let parsed: Vec<&str> = arguments.split_ascii_whitespace().collect();
    world.run(&parsed)
}

#[then("the repair was requested once")]
fn then_repaired_once(world: &RepairWorld) -> Result<(), String> {
    let repairs = world.invocation()?.repairs;
    if repairs == 1 {
        Ok(())
    } else {
        Err(format!("expected exactly one repair, found {repairs}"))
    }
}

#[then("the repair was not requested")]
fn then_not_repaired(world: &RepairWorld) -> Result<(), String> {
    let repairs = world.invocation()?.repairs;
    if repairs == 0 {
        Ok(())
    } else {
        Err(format!("expected no repair, found {repairs}"))
    }
}

#[then("Dylint ran once")]
fn then_linted_once(world: &RepairWorld) -> Result<(), String> {
    let runs = world.invocation()?.runs;
    if runs == 1 {
        Ok(())
    } else {
        Err(format!("expected exactly one Dylint run, found {runs}"))
    }
}

#[then("Dylint was not run")]
fn then_not_linted(world: &RepairWorld) -> Result<(), String> {
    let runs = world.invocation()?.runs;
    if runs == 0 {
        Ok(())
    } else {
        Err(format!("expected no Dylint run, found {runs}"))
    }
}

#[then("the outcome is Linted")]
fn then_outcome_is_linted(world: &RepairWorld) -> Result<(), String> {
    let code = world.invocation()?.code();
    if code == 0 {
        Ok(())
    } else {
        Err(format!("expected a linted outcome, found exit code {code}"))
    }
}

#[then("the exit code is {expected:i32} with reason key \"{key}\"")]
fn then_exit_code_with_key(world: &RepairWorld, expected: i32, key: &str) -> Result<(), String> {
    let invocation = world.invocation()?;
    let code = invocation.code();
    if code != expected {
        return Err(format!(
            "expected exit code {expected}, found {code}, stderr: {}",
            invocation.stderr
        ));
    }
    then_stderr_reports_key(world, key)
}

#[then("stderr reports the reason key \"{key}\"")]
fn then_stderr_reports_key(world: &RepairWorld, key: &str) -> Result<(), String> {
    let invocation = world.invocation()?;
    let expected = format!("whitaker: error[{key}]:");
    if invocation.stderr.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "stderr should report {expected}, got {:?}",
            invocation.stderr
        ))
    }
}

#[then("standard output is empty")]
fn then_stdout_is_empty(world: &RepairWorld) -> Result<(), String> {
    let stdout = world.invocation()?.stdout.clone();
    if stdout.is_empty() {
        Ok(())
    } else {
        Err(format!("standard output must stay pure, got {stdout:?}"))
    }
}

#[then("standard error contains the installer marker")]
fn then_stderr_contains_marker(world: &RepairWorld) -> Result<(), String> {
    let stderr = world.invocation()?.stderr.clone();
    if stderr.contains(MARKER) {
        Ok(())
    } else {
        Err(format!(
            "the installer marker must reach standard error, got {stderr:?}"
        ))
    }
}

#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 0)]
fn scenario_repairs_once_before_linting(world: RepairWorld) { let _ = world; }

#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 1)]
fn scenario_ready_is_not_repaired(world: RepairWorld) { let _ = world; }

#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 2)]
fn scenario_no_install_refuses(world: RepairWorld) { let _ = world; }

#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 3)]
fn scenario_user_path_is_never_repaired(world: RepairWorld) { let _ = world; }

#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 4)]
fn scenario_non_converging_repair(world: RepairWorld) { let _ = world; }

#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 5)]
fn scenario_failed_repair(world: RepairWorld) { let _ = world; }
