//! Behavioural tests for new installer workflows.
//!
//! These scenarios test the --skip-deps, --no-update, and --skip-wrapper flags
//! added to support standalone installation without a pre-cloned repository.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::toolchain::parse_toolchain_channel;

#[derive(Default)]
struct WorkflowWorld {
    args: RefCell<Vec<String>>,
    output: RefCell<Option<Output>>,
    skip_assertions: Cell<bool>,
    requires_toolchain: Cell<bool>,
    _temp_dir: RefCell<Option<TempDir>>,
}

#[fixture]
fn world() -> WorkflowWorld {
    WorkflowWorld::default()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest dir should have parent")
        .to_owned()
}

fn pinned_toolchain_channel() -> String {
    let toolchain_path = workspace_root().join("rust-toolchain.toml");
    let contents = std::fs::read_to_string(&toolchain_path).unwrap_or_else(|err| {
        panic!(
            "failed to read rust-toolchain.toml at {}: {err}",
            toolchain_path.display()
        )
    });
    parse_toolchain_channel(&contents).unwrap_or_else(|err| {
        panic!(
            "failed to parse rust-toolchain.toml at {}: {err}",
            toolchain_path.display()
        )
    })
}

fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn skip_scenario_when_toolchain_missing(world: &WorkflowWorld, channel: &str) {
    if !is_toolchain_installed(channel) {
        eprintln!(
            "Skipping scenario because rustup toolchain '{}' is not installed.",
            channel
        );
        world.skip_assertions.set(true);
        rstest_bdd::skip!(
            "rustup toolchain '{channel}' is not installed.",
            channel = channel
        );
    }
}

fn ensure_required_toolchain_available(world: &WorkflowWorld) -> Option<String> {
    let channel = pinned_toolchain_channel();
    world.requires_toolchain.set(true);

    skip_scenario_when_toolchain_missing(world, &channel);

    if world.skip_assertions.get() {
        None
    } else {
        Some(channel)
    }
}

macro_rules! skip_if_needed {
    ($world:expr) => {
        if $world.skip_assertions.get() {
            return;
        }
    };
}

fn setup_temp_dir(world: &WorkflowWorld) -> String {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let target_dir = temp_dir.path().to_string_lossy().to_string();
    world._temp_dir.replace(Some(temp_dir));
    target_dir
}

fn get_output(world: &WorkflowWorld) -> std::cell::Ref<'_, Output> {
    let output = world.output.borrow();
    std::cell::Ref::map(output, |opt| opt.as_ref().expect("output not set"))
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

fn given_dry_run_with_flag(world: &WorkflowWorld, flag: &str) {
    let Some(channel) = ensure_required_toolchain_available(world) else {
        return;
    };

    world.args.replace(vec![
        "--dry-run".to_owned(),
        "--toolchain".to_owned(),
        channel,
        flag.to_owned(),
    ]);
}

#[given("the installer is invoked with dry-run and skip-deps")]
fn given_dry_run_skip_deps(world: &WorkflowWorld) {
    given_dry_run_with_flag(world, "--skip-deps");
}

#[given("the installer is invoked with dry-run and no-update")]
fn given_dry_run_no_update(world: &WorkflowWorld) {
    given_dry_run_with_flag(world, "--no-update");
}

#[given("the installer is invoked with dry-run and skip-wrapper")]
fn given_dry_run_skip_wrapper(world: &WorkflowWorld) {
    given_dry_run_with_flag(world, "--skip-wrapper");
}

#[given("the installer is invoked with skip-wrapper to a temporary directory")]
fn given_skip_wrapper_install(world: &WorkflowWorld) {
    let Some(_channel) = ensure_required_toolchain_available(world) else {
        return;
    };

    let target_dir = setup_temp_dir(world);

    // Use --skip-deps to avoid slow dependency downloads during test.
    world.args.replace(vec![
        "--jobs".to_owned(),
        "1".to_owned(),
        "--target-dir".to_owned(),
        target_dir,
        "--skip-wrapper".to_owned(),
        "--skip-deps".to_owned(),
    ]);
}

#[when("the installer CLI is run")]
fn when_installer_cli_run(world: &WorkflowWorld) {
    skip_if_needed!(world);

    let args = world.args.borrow();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
    cmd.args(args.iter());
    cmd.current_dir(workspace_root());

    let output = cmd.output().expect("failed to run whitaker-installer");
    world.output.replace(Some(output));
}

#[then("the CLI exits successfully")]
fn then_cli_exits_successfully(world: &WorkflowWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[then("installation succeeds or is skipped")]
fn then_installation_succeeds_or_is_skipped(world: &WorkflowWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    assert!(
        output.status.success(),
        "installation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[then("dry-run output shows skip_deps is true")]
fn then_skip_deps_is_true(world: &WorkflowWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("Skip deps: true"),
        "expected skip_deps to be true in output, stderr: {stderr}"
    );
}

#[then("dry-run output shows no_update is true")]
fn then_no_update_is_true(world: &WorkflowWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("No update: true"),
        "expected no_update to be true in output, stderr: {stderr}"
    );
}

#[then("dry-run output shows skip_wrapper is true")]
fn then_skip_wrapper_is_true(world: &WorkflowWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("Skip wrapper: true"),
        "expected skip_wrapper to be true in output, stderr: {stderr}"
    );
}

#[then("output includes DYLINT_LIBRARY_PATH instructions")]
fn then_output_includes_library_path_instructions(world: &WorkflowWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("DYLINT_LIBRARY_PATH"),
        "expected DYLINT_LIBRARY_PATH instructions in output, stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/installer.feature", index = 15)]
fn scenario_dry_run_skip_deps(world: WorkflowWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/installer.feature", index = 16)]
fn scenario_dry_run_no_update(world: WorkflowWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/installer.feature", index = 17)]
fn scenario_dry_run_skip_wrapper(world: WorkflowWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/installer.feature", index = 18)]
fn scenario_skip_wrapper_outputs_shell_snippet(world: WorkflowWorld) {
    let _ = world;
}
