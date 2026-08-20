//! Behavioural tests for new installer workflows.
//!
//! These scenarios test the --skip-deps, --no-update, and --skip-wrapper flags
//! added to support standalone installation without a pre-cloned repository.

use std::{
    cell::{Cell, RefCell},
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use whitaker_installer::{test_support::TEST_STAGE_SUITE_ENV, toolchain::parse_toolchain_channel};

#[derive(Default)]
struct WorkflowWorld {
    args: RefCell<Vec<String>>,
    output: RefCell<Option<Output>>,
    skip_assertions: Cell<bool>,
    requires_toolchain: Cell<bool>,
    use_test_staged_suite: Cell<bool>,
    /// Owns the scenario's temporary target directory so it outlives the run.
    temp_dir: RefCell<Option<TempDir>>,
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> WorkflowWorld { WorkflowWorld::default() }

fn workspace_root() -> Result<PathBuf, String> {
    PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| String::from("manifest dir should have parent"))
}

fn pinned_toolchain_channel() -> Result<String, String> {
    let toolchain_path = workspace_root()?.join("rust-toolchain.toml");
    let contents = std::fs::read_to_string(&toolchain_path).map_err(|err| {
        format!(
            "failed to read rust-toolchain.toml at {}: {err}",
            toolchain_path.display()
        )
    })?;
    parse_toolchain_channel(&contents).map_err(|err| {
        format!(
            "failed to parse rust-toolchain.toml at {}: {err}",
            toolchain_path.display()
        )
    })
}

fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Reports a skipped scenario on stderr without tripping `print_stderr`.
fn report_skip(reason: &str) -> Result<(), String> {
    writeln!(std::io::stderr(), "{reason}")
        .map_err(|error| format!("failed to report skipped scenario: {error}"))
}

fn skip_scenario_when_toolchain_missing(
    world: &WorkflowWorld,
    channel: &str,
) -> Result<(), String> {
    if !is_toolchain_installed(channel) {
        report_skip(&format!(
            "Skipping scenario because rustup toolchain '{channel}' is not installed."
        ))?;
        world.skip_assertions.set(true);
        rstest_bdd::skip!(
            "rustup toolchain '{channel}' is not installed.",
            channel = channel
        );
    }
    Ok(())
}

fn ensure_required_toolchain_available(world: &WorkflowWorld) -> Result<Option<String>, String> {
    let channel = pinned_toolchain_channel()?;
    world.requires_toolchain.set(true);

    skip_scenario_when_toolchain_missing(world, &channel)?;

    Ok((!world.skip_assertions.get()).then_some(channel))
}

macro_rules! skip_if_needed {
    ($world:expr) => {
        if $world.skip_assertions.get() {
            return Ok(());
        }
    };
}

fn setup_temp_dir(world: &WorkflowWorld) -> Result<String, String> {
    let temp_dir = TempDir::new().map_err(|error| format!("failed to create temp dir: {error}"))?;
    let target_dir = temp_dir.path().to_string_lossy().to_string();
    world.temp_dir.replace(Some(temp_dir));
    Ok(target_dir)
}

/// Borrows the captured CLI output, failing when no command has run yet.
fn get_output(world: &WorkflowWorld) -> Result<std::cell::Ref<'_, Output>, String> {
    let output = world.output.borrow();
    std::cell::Ref::filter_map(output, Option::as_ref)
        .map_err(|_| String::from("CLI output not set; run the installer step first"))
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

fn given_dry_run_with_flag(world: &WorkflowWorld, flag: &str) -> Result<(), String> {
    let Some(channel) = ensure_required_toolchain_available(world)? else {
        return Ok(());
    };

    world.args.replace(vec![
        "--dry-run".to_owned(),
        "--toolchain".to_owned(),
        channel,
        flag.to_owned(),
    ]);
    Ok(())
}

#[given("the installer is invoked with dry-run and skip-deps")]
fn given_dry_run_skip_deps(world: &WorkflowWorld) -> Result<(), String> {
    given_dry_run_with_flag(world, "--skip-deps")
}

#[given("the installer is invoked with dry-run and no-update")]
fn given_dry_run_no_update(world: &WorkflowWorld) -> Result<(), String> {
    given_dry_run_with_flag(world, "--no-update")
}

#[given("the installer is invoked with dry-run and skip-wrapper")]
fn given_dry_run_skip_wrapper(world: &WorkflowWorld) -> Result<(), String> {
    given_dry_run_with_flag(world, "--skip-wrapper")
}

#[given("the installer is invoked with skip-wrapper to a temporary directory")]
fn given_skip_wrapper_install(world: &WorkflowWorld) -> Result<(), String> {
    if ensure_required_toolchain_available(world)?.is_none() {
        return Ok(());
    }

    let target_dir = setup_temp_dir(world)?;
    world.use_test_staged_suite.set(true);

    // The behavioural test sets a dedicated env var so the installer stages a
    // synthetic suite library instead of recursively building the workspace.
    // Use --skip-deps to avoid slow dependency downloads during test.
    world.args.replace(vec![
        "--target-dir".to_owned(),
        target_dir,
        "--skip-wrapper".to_owned(),
        "--skip-deps".to_owned(),
    ]);
    Ok(())
}

#[when("the installer CLI is run")]
fn when_installer_cli_run(world: &WorkflowWorld) -> Result<(), String> {
    skip_if_needed!(world);

    let args = world.args.borrow();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
    cmd.args(args.iter());
    cmd.current_dir(workspace_root()?);
    if world.use_test_staged_suite.get() {
        cmd.env(TEST_STAGE_SUITE_ENV, "1");
    }

    let output = cmd
        .output()
        .map_err(|error| format!("failed to run whitaker-installer: {error}"))?;
    world.output.replace(Some(output));
    Ok(())
}

#[then("the CLI exits successfully")]
fn then_cli_exits_successfully(world: &WorkflowWorld) -> Result<(), String> {
    skip_if_needed!(world);

    let output = get_output(world)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "expected success, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[then("installation succeeds or is skipped")]
fn then_installation_succeeds_or_is_skipped(world: &WorkflowWorld) -> Result<(), String> {
    skip_if_needed!(world);

    let output = get_output(world)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "installation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[then("dry-run output shows skip_deps is true")]
fn then_skip_deps_is_true(world: &WorkflowWorld) -> Result<(), String> {
    skip_if_needed!(world);

    let output = get_output(world)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("Skip deps: true") {
        Ok(())
    } else {
        Err(format!(
            "expected skip_deps to be true in output, stderr: {stderr}"
        ))
    }
}

#[then("dry-run output shows no_update is true")]
fn then_no_update_is_true(world: &WorkflowWorld) -> Result<(), String> {
    skip_if_needed!(world);

    let output = get_output(world)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("No update: true") {
        Ok(())
    } else {
        Err(format!(
            "expected no_update to be true in output, stderr: {stderr}"
        ))
    }
}

#[then("dry-run output shows skip_wrapper is true")]
fn then_skip_wrapper_is_true(world: &WorkflowWorld) -> Result<(), String> {
    skip_if_needed!(world);

    let output = get_output(world)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("Skip wrapper: true") {
        Ok(())
    } else {
        Err(format!(
            "expected skip_wrapper to be true in output, stderr: {stderr}"
        ))
    }
}

#[then("output includes DYLINT_LIBRARY_PATH instructions")]
fn then_output_includes_library_path_instructions(world: &WorkflowWorld) -> Result<(), String> {
    skip_if_needed!(world);

    let output = get_output(world)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("DYLINT_LIBRARY_PATH") {
        Ok(())
    } else {
        Err(format!(
            "expected DYLINT_LIBRARY_PATH instructions in output, stderr: {stderr}"
        ))
    }
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/installer.feature", index = 15)]
fn scenario_dry_run_skip_deps(world: WorkflowWorld) { let _ = world; }

#[scenario(path = "tests/features/installer.feature", index = 16)]
fn scenario_dry_run_no_update(world: WorkflowWorld) { let _ = world; }

#[scenario(path = "tests/features/installer.feature", index = 17)]
fn scenario_dry_run_skip_wrapper(world: WorkflowWorld) { let _ = world; }

#[scenario(path = "tests/features/installer.feature", index = 18)]
fn scenario_skip_wrapper_outputs_shell_snippet(world: WorkflowWorld) { let _ = world; }
