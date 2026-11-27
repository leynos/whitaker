//! In-process scenarios for lazy repair in `whitaker check`.
//!
//! The scenarios drive the real router and command with fake ports, so the
//! decision under test is the one that ships: no `cargo`, no network and no
//! filesystem beyond the staged suite a prepared value must prove. The fake
//! repair writes the installer's `suite-source` marker into the stream it is
//! handed, which is standard error, and the captured standard output must stay
//! empty — during `check`, every byte on standard output has to come from the
//! Dylint child.

use std::{
    cell::{Ref, RefCell},
    ffi::OsString,
};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker::{
    cli::{
        self,
        CommandStreams,
        check::exit::CheckOutcome,
        exit::{Outcome, exit_code_for},
    },
    ports::Services,
};

#[path = "behaviour_check_repair/fakes.rs"]
mod fakes;
use fakes::{
    Answer,
    FakeCheck,
    FakeInstall,
    SuiteLibrary,
    UnusedInventory,
    UnusedPackages,
    UnusedRunner,
    answer_for,
};

/// The state one scenario accumulates.
#[derive(Default)]
struct RepairWorld {
    /// What the readiness check answers, in order; the last answer repeats.
    answers: RefCell<Vec<Answer>>,
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
    outcome: Outcome,
    /// Everything Whitaker wrote to standard output.
    stdout: String,
    /// Everything Whitaker wrote to standard error.
    stderr: String,
    /// How many repairs the check port was asked for.
    repairs: usize,
    /// How many Dylint runs the check port was asked for.
    runs: usize,
    /// What the flow asked the check port for, in call order.
    trace: Vec<String>,
}

impl Invocation {
    /// The process exit code the outcome maps to.
    const fn code(&self) -> i32 { exit_code_for(&self.outcome) }
}

impl RepairWorld {
    /// The recorded invocation, if the scenario ran one.
    fn invocation(&self) -> Result<Ref<'_, Invocation>, String> {
        Ref::filter_map(self.recorded.borrow(), Option::as_ref)
            .map_err(|_| "the check invocation should be recorded".to_owned())
    }

    /// Run `whitaker check` with `arguments` through the real router.
    fn run(&self, arguments: &[&str]) -> Result<(), String> {
        let repair_fails = *self.repair_fails.borrow();
        let user_managed = *self.user_managed.borrow();
        let check = FakeCheck::new(
            self.answers.borrow().clone(),
            SuiteLibrary::staged()?,
            repair_fails,
            user_managed,
        );
        let install = FakeInstall::default();
        let services = Services {
            install: &install,
            inventory: &UnusedInventory,
            lint_runner: &UnusedRunner,
            check: &check,
            packages: &UnusedPackages,
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
            repairs: check.repairs(),
            runs: check.runs(),
            trace: check.trace(),
        }));
        Ok(())
    }
}

/// Provide the scenario world fixture.
#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> RepairWorld { RepairWorld::default() }

/// Record one scripted readiness answer.
#[given("preflight reports \"{reason}\"")]
fn given_preflight_reports(world: &RepairWorld, reason: &str) -> Result<(), String> {
    world.answers.borrow_mut().push(answer_for(reason)?);
    Ok(())
}

/// Record two scripted readiness answers, consumed in order.
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

/// Script the repair to succeed.
#[given("the repair succeeds")]
fn given_repair_succeeds(world: &RepairWorld) { world.repair_fails.replace(false); }

/// Script the repair to fail.
#[given("the repair fails")]
fn given_repair_fails(world: &RepairWorld) { world.repair_fails.replace(true); }

/// Script the library path as one the user manages.
#[given("a user-managed library path")]
fn given_user_managed_path(world: &RepairWorld) { world.user_managed.replace(true); }

/// Run the command with its default options.
#[when("check runs with default options")]
fn when_check_runs(world: &RepairWorld) -> Result<(), String> { world.run(&[] as &[&str]) }

/// Run the command with `arguments` from the feature file.
#[when("check runs with \"{arguments}\"")]
fn when_check_runs_with(world: &RepairWorld, arguments: &str) -> Result<(), String> {
    let parsed: Vec<&str> = arguments.split_ascii_whitespace().collect();
    world.run(&parsed)
}

/// Require exactly one repair to have been requested.
#[then("the repair was requested once")]
fn then_repaired_once(world: &RepairWorld) -> Result<(), String> {
    let repairs = world.invocation()?.repairs;
    if repairs == 1 {
        Ok(())
    } else {
        Err(format!("expected exactly one repair, found {repairs}"))
    }
}

/// Require that no repair was requested.
#[then("the repair was not requested")]
fn then_not_repaired(world: &RepairWorld) -> Result<(), String> {
    let repairs = world.invocation()?.repairs;
    if repairs == 0 {
        Ok(())
    } else {
        Err(format!("expected no repair, found {repairs}"))
    }
}

/// Require exactly one Dylint run.
#[then("Dylint ran once")]
fn then_linted_once(world: &RepairWorld) -> Result<(), String> {
    let runs = world.invocation()?.runs;
    if runs == 1 {
        Ok(())
    } else {
        Err(format!("expected exactly one Dylint run, found {runs}"))
    }
}

/// Require that Dylint did not run.
#[then("Dylint was not run")]
fn then_not_linted(world: &RepairWorld) -> Result<(), String> {
    let runs = world.invocation()?.runs;
    if runs == 0 {
        Ok(())
    } else {
        Err(format!("expected no Dylint run, found {runs}"))
    }
}

/// Require the flow to have traced exactly `trace`, in order.
///
/// A repair count and a run count do not establish order: Dylint could run
/// before the repaired installation was rechecked and the counts would still
/// match. The trace names each preflight answer, so the scenario can require
/// `NotInstalled -> repair -> Ready -> run`.
#[then("the check traced \"{trace}\"")]
fn then_check_traced(world: &RepairWorld, trace: &str) -> Result<(), String> {
    let invocation = world.invocation()?;
    let expected: Vec<String> = trace.split(" -> ").map(str::to_owned).collect();
    if invocation.trace == expected {
        Ok(())
    } else {
        Err(format!(
            "expected the check to trace {expected:?}, found {:?}",
            invocation.trace
        ))
    }
}

/// Require the router to have reported a successful lint.
#[then("the outcome is Linted")]
fn then_outcome_is_linted(world: &RepairWorld) -> Result<(), String> {
    let outcome = world.invocation()?.outcome;
    if outcome == Outcome::Check(CheckOutcome::Linted(0)) {
        Ok(())
    } else {
        Err(format!("expected a linted outcome, found {outcome:?}"))
    }
}

/// Require the exit code and its reason key.
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

/// Require standard error to report the reason key `key`.
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

/// Require standard output to have stayed empty.
#[then("standard output is empty")]
fn then_stdout_is_empty(world: &RepairWorld) -> Result<(), String> {
    let stdout = world.invocation()?.stdout.clone();
    if stdout.is_empty() {
        Ok(())
    } else {
        Err(format!("standard output must stay pure, got {stdout:?}"))
    }
}

/// Require the installer marker to have reached standard error.
#[then("standard error contains the installer marker")]
fn then_stderr_contains_marker(world: &RepairWorld) -> Result<(), String> {
    let stderr = world.invocation()?.stderr.clone();
    if stderr.contains(fakes::MARKER) {
        Ok(())
    } else {
        Err(format!(
            "the installer marker must reach standard error, got {stderr:?}"
        ))
    }
}

/// Bind the converged repair-and-lint scenario.
#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 0)]
fn scenario_repairs_once_before_linting(world: RepairWorld) { let _ = world; }

/// Bind the ready installation not repaired scenario.
#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 1)]
fn scenario_ready_is_not_repaired(world: RepairWorld) { let _ = world; }

/// Bind the `--no-install` refusal scenario.
#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 2)]
fn scenario_no_install_refuses(world: RepairWorld) { let _ = world; }

/// Bind the user-managed path never repaired scenario.
#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 3)]
fn scenario_user_path_is_never_repaired(world: RepairWorld) { let _ = world; }

/// Bind the non-converging repair scenario.
#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 4)]
fn scenario_non_converging_repair(world: RepairWorld) { let _ = world; }

/// Bind the failed repair scenario.
#[scenario(path = "tests/features/whitaker_check_repair.feature", index = 5)]
fn scenario_failed_repair(world: RepairWorld) { let _ = world; }
