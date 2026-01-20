//! Behavioural tests for toolchain auto-install functionality.
//!
//! These scenarios test that the installer correctly detects and, if needed,
//! auto-installs the pinned toolchain from rust-toolchain.toml.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::toolchain::parse_toolchain_channel;

#[derive(Default)]
struct ToolchainWorld {
    args: RefCell<Vec<String>>,
    output: RefCell<Option<Output>>,
    skip_assertions: Cell<bool>,
    _temp_dir: RefCell<Option<TempDir>>,
}

#[fixture]
fn world() -> ToolchainWorld {
    ToolchainWorld::default()
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

fn skip_scenario_when_toolchain_missing(world: &ToolchainWorld, channel: &str) {
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

fn setup_temp_dir(world: &ToolchainWorld) -> String {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let target_dir = temp_dir.path().to_string_lossy().to_string();
    world._temp_dir.replace(Some(temp_dir));
    target_dir
}

fn get_output(world: &ToolchainWorld) -> std::cell::Ref<'_, Output> {
    let output = world.output.borrow();
    std::cell::Ref::map(output, |opt| opt.as_ref().expect("output not set"))
}

macro_rules! skip_if_needed {
    ($world:expr) => {
        if $world.skip_assertions.get() {
            return;
        }
    };
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("the installer is invoked with auto-detect toolchain")]
fn given_auto_detect_toolchain(world: &ToolchainWorld) {
    let channel = pinned_toolchain_channel();
    skip_scenario_when_toolchain_missing(world, &channel);

    let target_dir = setup_temp_dir(world);

    // No --toolchain flag - let the installer detect from rust-toolchain.toml
    world.args.replace(vec![
        "--dry-run".to_owned(),
        "--target-dir".to_owned(),
        target_dir,
    ]);
}

#[given("the installer is invoked with auto-detect toolchain in quiet mode")]
fn given_auto_detect_toolchain_quiet(world: &ToolchainWorld) {
    let channel = pinned_toolchain_channel();
    skip_scenario_when_toolchain_missing(world, &channel);

    let target_dir = setup_temp_dir(world);

    world.args.replace(vec![
        "--dry-run".to_owned(),
        "--quiet".to_owned(),
        "--target-dir".to_owned(),
        target_dir,
    ]);
}

#[given("the installer is invoked with auto-detect toolchain to a temporary directory")]
fn given_auto_detect_toolchain_install(world: &ToolchainWorld) {
    let channel = pinned_toolchain_channel();
    skip_scenario_when_toolchain_missing(world, &channel);

    let target_dir = setup_temp_dir(world);

    // No --toolchain flag - use auto-detected toolchain
    world.args.replace(vec![
        "--jobs".to_owned(),
        "1".to_owned(),
        "--target-dir".to_owned(),
        target_dir,
    ]);
}

#[when("the installer CLI is run")]
fn when_installer_cli_run(world: &ToolchainWorld) {
    skip_if_needed!(world);

    let args = world.args.borrow();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
    cmd.args(args.iter());
    cmd.current_dir(workspace_root());

    let output = cmd.output().expect("failed to run whitaker-installer");
    world.output.replace(Some(output));
}

#[then("the CLI exits successfully")]
fn then_cli_exits_successfully(world: &ToolchainWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[then("dry-run output shows the detected toolchain")]
fn then_dry_run_shows_toolchain(world: &ToolchainWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected_channel = pinned_toolchain_channel();

    assert!(
        stderr.contains(&expected_channel),
        "expected toolchain '{expected_channel}' in output, stderr: {stderr}"
    );
}

#[then("no toolchain installation message is shown")]
fn then_no_install_message(world: &ToolchainWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("installed successfully"),
        "expected no installation message in output, stderr: {stderr}"
    );
}

#[then("installation succeeds or is skipped")]
fn then_installation_succeeds_or_is_skipped(world: &ToolchainWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    assert!(
        output.status.success(),
        "installation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[then("the suite library is staged")]
fn then_suite_library_is_staged(world: &ToolchainWorld) {
    skip_if_needed!(world);

    // Just verify the output indicates staging occurred
    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The installer outputs "Staged:" when libraries are staged
    assert!(
        stderr.contains("Staged:") || stderr.contains("whitaker_suite"),
        "expected staging output, stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/toolchain.feature", index = 0)]
fn scenario_auto_detect_toolchain_dry_run(world: ToolchainWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/toolchain.feature", index = 1)]
fn scenario_auto_detect_toolchain_quiet_mode(world: ToolchainWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/toolchain.feature", index = 2)]
fn scenario_auto_detect_toolchain_install(world: ToolchainWorld) {
    let _ = world;
}
