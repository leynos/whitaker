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
//!
//! Note: The install scenario downloads toolchains from the network and may be slow.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::toolchain::parse_toolchain_channel;

/// Non-existent toolchain channel used to exercise auto-install failure paths.
/// This channel name is intentionally invalid to trigger rustup installation failure.
const FAKE_TOOLCHAIN: &str = "nonexistent-toolchain-xyz-12345";

/// Output marker indicating successful library staging.
/// The installer outputs this text when libraries are being staged to the target directory.
const STAGING_OUTPUT_MARKER: &str = "Staging libraries to";

/// Output marker indicating successful toolchain installation.
/// The installer emits this message when a toolchain was auto-installed.
const TOOLCHAIN_INSTALLED_MARKER: &str = "installed successfully";

/// Canonical error marker for toolchain installation failures or missing toolchains.
/// The installer uses this phrase when the toolchain is not available.
const TOOLCHAIN_ERROR_MARKER: &str = "not installed";

/// Maximum number of output lines expected in quiet mode error scenarios.
/// Quiet mode should suppress progress messages, leaving only the error itself.
/// This threshold accounts for: error message line(s), blank lines, and minimal context.
const QUIET_MODE_MAX_LINES: usize = 5;

#[derive(Default)]
struct ToolchainWorld {
    args: RefCell<Vec<String>>,
    output: RefCell<Option<Output>>,
    skip_assertions: Cell<bool>,
    /// Holds the target temp directory to prevent cleanup until the test completes.
    temp_dir: RefCell<Option<TempDir>>,
    /// Isolated RUSTUP_HOME directory for auto-install scenarios.
    rustup_home: RefCell<Option<TempDir>>,
    /// Isolated CARGO_HOME directory for auto-install scenarios.
    cargo_home: RefCell<Option<TempDir>>,
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
    let contents =
        std::fs::read_to_string(&toolchain_path).expect("failed to read rust-toolchain.toml");
    parse_toolchain_channel(&contents).expect("failed to parse rust-toolchain.toml")
}

fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Checks if a toolchain is installed in an isolated rustup environment.
fn is_toolchain_installed_in_env(
    channel: &str,
    rustup_home: &TempDir,
    cargo_home: &TempDir,
) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .env("RUSTUP_HOME", rustup_home.path())
        .env("CARGO_HOME", cargo_home.path())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Marks the scenario to be skipped if the pinned toolchain is not installed.
// Used for dry-run scenarios that test detection rather than installation.
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

// Sets up isolated RUSTUP_HOME and CARGO_HOME directories for the scenario.
// This ensures the auto-install code path is exercised regardless of host state.
fn setup_isolated_rustup(world: &ToolchainWorld) {
    let rustup_home = TempDir::new().expect("failed to create RUSTUP_HOME temp dir");
    let cargo_home = TempDir::new().expect("failed to create CARGO_HOME temp dir");

    // Initialize the isolated rustup environment by running `rustup show`.
    // This creates the necessary settings files that rustup expects to exist.
    // We set RUSTUP_AUTO_INSTALL=0 to prevent rustup from auto-installing
    // any toolchain during initialization.
    // We also clear RUSTUP_TOOLCHAIN and run from the temp directory to avoid
    // rust-toolchain.toml files affecting the initialization.
    let init_output = Command::new("rustup")
        .arg("show")
        .current_dir(rustup_home.path())
        .env("RUSTUP_HOME", rustup_home.path())
        .env("CARGO_HOME", cargo_home.path())
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .expect("failed to initialize isolated rustup environment");

    assert!(
        init_output.status.success(),
        "failed to initialize isolated rustup: {}",
        String::from_utf8_lossy(&init_output.stderr)
    );

    // Rustup expects to find itself at $CARGO_HOME/bin/rustup. Create a symlink
    // to the system rustup so that toolchain install succeeds.
    let cargo_bin = cargo_home.path().join("bin");
    std::fs::create_dir_all(&cargo_bin).expect("failed to create CARGO_HOME/bin");
    let rustup_path_output = Command::new("which")
        .arg("rustup")
        .output()
        .expect("failed to run which rustup");
    let rustup_path = String::from_utf8_lossy(&rustup_path_output.stdout)
        .trim()
        .to_string();
    std::os::unix::fs::symlink(&rustup_path, cargo_bin.join("rustup"))
        .expect("failed to symlink rustup to CARGO_HOME/bin");

    world.rustup_home.replace(Some(rustup_home));
    world.cargo_home.replace(Some(cargo_home));
}

fn setup_temp_dir(world: &ToolchainWorld) -> String {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let target_dir = temp_dir.path().to_string_lossy().to_string();
    world.temp_dir.replace(Some(temp_dir));
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
// Scenario setup helpers
// ---------------------------------------------------------------------------

// Sets up a dry-run scenario that requires the pinned toolchain to be installed.
// Skips the scenario if the toolchain is missing since dry-run does not install.
fn setup_dry_run_scenario(world: &ToolchainWorld, extra_args: &[&str]) {
    let channel = pinned_toolchain_channel();
    skip_scenario_when_toolchain_missing(world, &channel);

    let target_dir = setup_temp_dir(world);

    let mut args: Vec<String> = extra_args.iter().map(|s| (*s).to_owned()).collect();
    args.extend(["--target-dir".to_owned(), target_dir]);
    world.args.replace(args);
}

// Sets up an install scenario with isolated rustup environment.
// The isolated environment ensures the auto-install code path is exercised.
fn setup_install_scenario(world: &ToolchainWorld, extra_args: &[&str]) {
    setup_isolated_rustup(world);
    let target_dir = setup_temp_dir(world);

    let mut args: Vec<String> = extra_args.iter().map(|s| (*s).to_owned()).collect();
    args.extend(["--target-dir".to_owned(), target_dir]);
    world.args.replace(args);
}

// Sets up a failure scenario using a non-existent toolchain.
fn setup_failure_scenario(world: &ToolchainWorld, extra_args: &[&str]) {
    let target_dir = setup_temp_dir(world);

    let mut args: Vec<String> = extra_args.iter().map(|s| (*s).to_owned()).collect();
    args.extend([
        "--toolchain".to_owned(),
        FAKE_TOOLCHAIN.to_owned(),
        "--target-dir".to_owned(),
        target_dir,
    ]);
    world.args.replace(args);
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("the installer is invoked with auto-detect toolchain")]
fn given_auto_detect_toolchain(world: &ToolchainWorld) {
    setup_dry_run_scenario(world, &["--dry-run"]);
}

#[given("the installer is invoked with auto-detect toolchain in quiet mode")]
fn given_auto_detect_toolchain_quiet(world: &ToolchainWorld) {
    setup_dry_run_scenario(world, &["--dry-run", "--quiet"]);
}

#[given("the installer is invoked with auto-detect toolchain to a temporary directory")]
fn given_auto_detect_toolchain_install(world: &ToolchainWorld) {
    setup_install_scenario(world, &["--jobs", "1", "--skip-deps"]);
}

#[given("the installer is invoked with isolated rustup to force auto-install")]
fn given_isolated_rustup_auto_install(world: &ToolchainWorld) {
    setup_install_scenario(world, &["--jobs", "1", "--skip-deps"]);
}

#[given("the installer is invoked with isolated rustup in quiet mode")]
fn given_isolated_rustup_quiet(world: &ToolchainWorld) {
    setup_install_scenario(world, &["--jobs", "1", "--quiet", "--skip-deps"]);
}

#[given("the installer is invoked with a non-existent toolchain")]
fn given_nonexistent_toolchain(world: &ToolchainWorld) {
    setup_failure_scenario(world, &["--dry-run"]);
}

#[given("the installer is invoked with a non-existent toolchain in quiet mode")]
fn given_nonexistent_toolchain_quiet(world: &ToolchainWorld) {
    setup_failure_scenario(world, &["--dry-run", "--quiet"]);
}

#[when("the installer CLI is run")]
fn when_installer_cli_run(world: &ToolchainWorld) {
    skip_if_needed!(world);

    let args = world.args.borrow();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
    cmd.args(args.iter());
    cmd.current_dir(workspace_root());

    // Set isolated rustup environment if configured for this scenario
    if let Some(ref rustup_home) = *world.rustup_home.borrow() {
        cmd.env("RUSTUP_HOME", rustup_home.path());
        // Disable rustup's auto-install feature so that the installer's
        // is_installed check doesn't silently install the toolchain. This
        // ensures the explicit install_toolchain_with call is triggered.
        cmd.env("RUSTUP_AUTO_INSTALL", "0");
        // Clear RUSTUP_TOOLCHAIN which may be inherited from the test runner.
        cmd.env_remove("RUSTUP_TOOLCHAIN");
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
        !stderr.contains(TOOLCHAIN_INSTALLED_MARKER),
        "expected no installation message in output, stderr: {stderr}"
    );
}

#[then("the toolchain installation message is shown")]
fn then_install_message_shown(world: &ToolchainWorld) {
    skip_if_needed!(world);

    let output = get_output(world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains(TOOLCHAIN_INSTALLED_MARKER),
        "expected '{TOOLCHAIN_INSTALLED_MARKER}' in output, stderr: {stderr}"
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

    // Verify the toolchain was actually installed in the isolated environment
    assert_toolchain_installed_in_isolated_env(world);
}

#[then("the toolchain is installed in the isolated environment")]
fn then_toolchain_installed_in_isolated_env(world: &ToolchainWorld) {
    skip_if_needed!(world);
    assert_toolchain_installed_in_isolated_env(world);
}

// Helper to verify the toolchain is installed in the isolated rustup environment.
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
        "expected '{TOOLCHAIN_ERROR_MARKER}' in stderr: {stderr}"
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
