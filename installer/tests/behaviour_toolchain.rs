//! Behavioural tests for toolchain auto-install functionality.
//!
//! These scenarios test that the installer correctly detects and, if needed,
//! auto-installs the pinned toolchain from rust-toolchain.toml.
//!
//! The tests include:
//! - Dry-run scenarios that test toolchain detection (skipped if toolchain missing)
//! - Install scenarios that exercise auto-install using an isolated rustup
//!   environment (RUSTUP_HOME/CARGO_HOME set to temp directories)
//! - Failure scenarios that test error handling with a non-existent toolchain

mod support;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use std::process::{Command, Output};
use support::{
    is_toolchain_installed, is_toolchain_installed_in_env, pinned_toolchain_channel,
    setup_isolated_rustup, workspace_root,
};
use tempfile::TempDir;

/// Non-existent toolchain channel used to exercise auto-install failure paths.
const FAKE_TOOLCHAIN: &str = "nonexistent-nightly-2024-01-01";

/// Output marker indicating successful library staging.
const STAGING_OUTPUT_MARKER: &str = "Staging libraries to";

/// Output marker indicating successful toolchain installation.
const TOOLCHAIN_INSTALLED_MARKER: &str = "installed successfully";

/// Canonical error marker for toolchain installation failures.
const TOOLCHAIN_ERROR_MARKER: &str = "installation failed";

/// Maximum output lines expected in quiet mode error scenarios.
const QUIET_MODE_MAX_LINES: usize = 5;

#[derive(Default)]
struct ToolchainWorld {
    args: RefCell<Vec<String>>,
    output: RefCell<Option<Output>>,
    should_skip_assertions: Cell<bool>,
    temp_dir: RefCell<Option<TempDir>>,
    rustup_home: RefCell<Option<TempDir>>,
    cargo_home: RefCell<Option<TempDir>>,
    pinned_channel: RefCell<String>,
}

fn get_output(world: &ToolchainWorld) -> std::cell::Ref<'_, Output> {
    let output = world.output.borrow();
    std::cell::Ref::map(output, |opt| opt.as_ref().expect("output not set"))
}

macro_rules! skip_if_needed {
    ($world:expr) => {
        if $world.should_skip_assertions.get() {
            return;
        }
    };
}

fn skip_scenario_when_toolchain_missing(world: &ToolchainWorld, channel: &str) {
    if !is_toolchain_installed(channel) {
        eprintln!("Skipping scenario: toolchain '{channel}' not installed.");
        world.should_skip_assertions.set(true);
        rstest_bdd::skip!("toolchain '{channel}' is not installed.", channel = channel);
    }
}

fn setup_temp_dir(world: &ToolchainWorld) -> String {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let target_dir = temp_dir.path().to_string_lossy().to_string();
    world.temp_dir.replace(Some(temp_dir));
    target_dir
}

fn setup_dry_run_scenario(world: &ToolchainWorld, extra_args: &[&str]) {
    let channel = pinned_toolchain_channel();
    skip_scenario_when_toolchain_missing(world, &channel);
    world.pinned_channel.replace(channel.clone());

    let target_dir = setup_temp_dir(world);
    let mut args: Vec<String> = extra_args.iter().map(|s| (*s).to_owned()).collect();
    args.extend(["--target-dir".to_owned(), target_dir]);
    world.args.replace(args);
}

fn setup_install_scenario(world: &ToolchainWorld, extra_args: &[&str]) {
    let env = setup_isolated_rustup();
    world.rustup_home.replace(Some(env.rustup_home));
    world.cargo_home.replace(Some(env.cargo_home));
    world.pinned_channel.replace(pinned_toolchain_channel());

    let target_dir = setup_temp_dir(world);
    let mut args: Vec<String> = extra_args.iter().map(|s| (*s).to_owned()).collect();
    args.extend(["--target-dir".to_owned(), target_dir]);
    world.args.replace(args);
}

fn setup_failure_scenario(world: &ToolchainWorld, extra_args: &[&str]) {
    // Use isolated rustup environment so the install failure path is exercised
    // without affecting the host system.
    let env = setup_isolated_rustup();
    world.rustup_home.replace(Some(env.rustup_home));
    world.cargo_home.replace(Some(env.cargo_home));

    let target_dir = setup_temp_dir(world);
    // Filter out --dry-run to exercise the real install path
    let mut args: Vec<String> = extra_args
        .iter()
        .filter(|s| **s != "--dry-run")
        .map(|s| (*s).to_owned())
        .collect();
    args.extend([
        "--toolchain".to_owned(),
        FAKE_TOOLCHAIN.to_owned(),
        "--target-dir".to_owned(),
        target_dir,
        "--skip-deps".to_owned(),
    ]);
    world.args.replace(args);
}

fn assert_toolchain_installed_in_isolated_env(world: &ToolchainWorld) {
    let rustup_home = world.rustup_home.borrow();
    let cargo_home = world.cargo_home.borrow();
    assert!(
        rustup_home.is_some() && cargo_home.is_some(),
        "isolated rustup environment must be configured for install scenario"
    );
    let rustup = rustup_home.as_ref().expect("rustup_home");
    let cargo = cargo_home.as_ref().expect("cargo_home");
    let channel = pinned_toolchain_channel();
    assert!(
        is_toolchain_installed_in_env(&channel, rustup, cargo),
        "toolchain '{channel}' was not installed in isolated environment"
    );
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[fixture]
fn world() -> ToolchainWorld {
    ToolchainWorld::default()
}

#[given("the installer is invoked with auto-detect toolchain")]
fn given_auto_detect_toolchain(world: &ToolchainWorld) {
    setup_dry_run_scenario(world, &["--dry-run"]);
}

#[given("the installer is invoked with auto-detect toolchain in quiet mode")]
fn given_auto_detect_toolchain_quiet(world: &ToolchainWorld) {
    setup_dry_run_scenario(world, &["--dry-run", "--quiet"]);
}

fn setup_auto_install_scenario(world: &ToolchainWorld) {
    // Skip auto-install tests on Windows - toolchain downloads are extremely slow
    // due to Windows Defender scanning and larger binaries. The code path is
    // identical to Linux; we're testing rustup behaviour rather than installer logic.
    if cfg!(windows) {
        eprintln!("Skipping auto-install scenario on Windows (toolchain downloads too slow).");
        world.should_skip_assertions.set(true);
        rstest_bdd::skip!("auto-install tests skipped on Windows");
    }
    // Use --skip-wrapper to prevent writing to the user's real ~/.local/bin.
    setup_install_scenario(world, &["--jobs", "1", "--skip-deps", "--skip-wrapper"]);
}

#[given("the installer is invoked with auto-detect toolchain to a temporary directory")]
fn given_auto_detect_toolchain_install(world: &ToolchainWorld) {
    setup_auto_install_scenario(world);
}

#[given("the installer is invoked with isolated rustup to force auto-install")]
fn given_isolated_rustup_auto_install(world: &ToolchainWorld) {
    setup_auto_install_scenario(world);
}

#[given("the installer is invoked with isolated rustup in quiet mode")]
fn given_isolated_rustup_quiet(world: &ToolchainWorld) {
    // Use --skip-wrapper to prevent writing to the user's real ~/.local/bin.
    setup_install_scenario(
        world,
        &["--jobs", "1", "--quiet", "--skip-deps", "--skip-wrapper"],
    );
}

#[given("the installer is invoked with a non-existent toolchain")]
fn given_nonexistent_toolchain(world: &ToolchainWorld) {
    setup_failure_scenario(world, &[]);
}

#[given("the installer is invoked with a non-existent toolchain in quiet mode")]
fn given_nonexistent_toolchain_quiet(world: &ToolchainWorld) {
    setup_failure_scenario(world, &["--quiet"]);
}

#[when("the installer CLI is run")]
fn when_installer_cli_run(world: &ToolchainWorld) {
    skip_if_needed!(world);

    let args = world.args.borrow();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
    cmd.args(args.iter());
    cmd.current_dir(workspace_root());

    // Sanitise rustup environment to prevent host settings from leaking
    // into tests: always disable auto-install and remove toolchain overrides
    cmd.env("RUSTUP_AUTO_INSTALL", "0");
    cmd.env_remove("RUSTUP_TOOLCHAIN");

    if let Some(ref rustup_home) = *world.rustup_home.borrow() {
        cmd.env("RUSTUP_HOME", rustup_home.path());
    }
    if let Some(ref cargo_home) = *world.cargo_home.borrow() {
        cmd.env("CARGO_HOME", cargo_home.path());
    }

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
    let expected_channel = world.pinned_channel.borrow().clone();
    assert!(
        stderr.contains(&expected_channel),
        "expected toolchain '{expected_channel}' in output, stderr: {stderr}"
    );
}

#[then("no toolchain installation message is shown")]
fn then_no_install_message(world: &ToolchainWorld) {
    skip_if_needed!(world);
    let output = get_output(world);
    let out = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let channel = world.pinned_channel.borrow().clone();
    let out_lc = out.to_lowercase();
    let needle = format!("toolchain {channel} installed successfully").to_lowercase();
    assert!(
        !(out_lc.contains(&needle)
            || out_lc.contains(&channel.to_lowercase())
                && out_lc.contains(TOOLCHAIN_INSTALLED_MARKER)),
        "expected no installation message for channel '{channel}' in output, got:\n{out}"
    );
}

#[then("the toolchain installation message is shown")]
fn then_install_message_shown(world: &ToolchainWorld) {
    skip_if_needed!(world);
    let output = get_output(world);
    let out = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let channel = world.pinned_channel.borrow().clone();
    let out_lc = out.to_lowercase();
    let needle = format!("toolchain {channel} installed successfully").to_lowercase();
    let ok = out_lc.contains(&needle)
        || (out_lc.contains("installed successfully") && out_lc.contains(&channel.to_lowercase()));
    assert!(
        ok,
        "expected success marker for channel '{channel}' in output, got:\n{out}"
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
    assert_toolchain_installed_in_isolated_env(world);
}

#[then("the toolchain is installed in the isolated environment")]
fn then_toolchain_installed_in_isolated_env(world: &ToolchainWorld) {
    skip_if_needed!(world);
    assert_toolchain_installed_in_isolated_env(world);
}

#[then("the suite library is staged")]
fn then_suite_library_is_staged(world: &ToolchainWorld) {
    skip_if_needed!(world);
    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(STAGING_OUTPUT_MARKER),
        "expected '{STAGING_OUTPUT_MARKER}' in staging output, stderr: {stderr}"
    );
}

#[then("the CLI exits with an error")]
fn then_cli_exits_with_error(world: &ToolchainWorld) {
    skip_if_needed!(world);
    let output = get_output(world);
    assert!(
        !output.status.success(),
        "expected failure exit code, but command succeeded"
    );
}

#[then("the error mentions toolchain installation failure")]
fn then_error_mentions_install_failure(world: &ToolchainWorld) {
    skip_if_needed!(world);
    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(TOOLCHAIN_ERROR_MARKER),
        "expected '{}' in stderr: {stderr}",
        TOOLCHAIN_ERROR_MARKER
    );
}

#[then("the error includes the toolchain name")]
fn then_error_includes_toolchain_name(world: &ToolchainWorld) {
    skip_if_needed!(world);
    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(FAKE_TOOLCHAIN),
        "expected toolchain name '{FAKE_TOOLCHAIN}' in error output, stderr: {stderr}"
    );
}

#[then("the error output is minimal")]
fn then_error_output_is_minimal(world: &ToolchainWorld) {
    skip_if_needed!(world);
    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line_count = stderr.lines().count();
    assert!(
        line_count <= QUIET_MODE_MAX_LINES,
        "expected at most {QUIET_MODE_MAX_LINES} lines in quiet mode, got {line_count}: {stderr}"
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

#[scenario(path = "tests/features/toolchain.feature", index = 3)]
fn scenario_auto_install_success_emits_message(world: ToolchainWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/toolchain.feature", index = 4)]
fn scenario_auto_install_success_quiet_mode(world: ToolchainWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/toolchain.feature", index = 5)]
fn scenario_auto_install_failure_reports_error(world: ToolchainWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/toolchain.feature", index = 6)]
fn scenario_auto_install_failure_quiet_mode(world: ToolchainWorld) {
    let _ = world;
}
