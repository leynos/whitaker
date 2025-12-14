//! End-to-end CLI behaviour tests for `whitaker-install`.
//!
//! These scenarios invoke the installer binary and validate dry-run output,
//! error handling, and (when possible) installation results.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::toolchain::parse_toolchain_channel;

#[derive(Default)]
struct CliWorld {
    args: RefCell<Vec<String>>,
    output: RefCell<Option<Output>>,
    skip_assertions: Cell<bool>,
    requires_toolchain: Cell<bool>,
    toolchain: RefCell<Option<String>>,
    // Keep temp_dir alive for the lifetime of the scenario.
    _temp_dir: RefCell<Option<TempDir>>,
}

#[fixture]
fn cli_world() -> CliWorld {
    CliWorld::default()
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

fn skip_scenario_when_toolchain_missing(cli_world: &CliWorld, channel: &str) {
    if !is_toolchain_installed(channel) {
        eprintln!(
            "Skipping scenario because rustup toolchain '{}' is not installed. Install this toolchain to run these tests.",
            channel
        );
        cli_world.skip_assertions.set(true);
        rstest_bdd::skip!(
            "rustup toolchain '{channel}' is not installed. Install this toolchain to run these tests.",
            channel = channel
        );
    }
}

fn ensure_toolchain_available(cli_world: &CliWorld) -> Option<String> {
    let channel = pinned_toolchain_channel();
    cli_world.toolchain.replace(Some(channel.clone()));

    if cli_world.requires_toolchain.get() {
        skip_scenario_when_toolchain_missing(cli_world, &channel);
    }

    if cli_world.skip_assertions.get() {
        None
    } else {
        Some(channel)
    }
}

/// Ensures a toolchain is available for scenarios that strictly require it.
/// Returns the channel if available, or None if the scenario should be skipped.
fn ensure_required_toolchain_available(cli_world: &CliWorld) -> Option<String> {
    cli_world.requires_toolchain.set(true);
    ensure_toolchain_available(cli_world)
}

macro_rules! skip_if_needed {
    ($cli_world:expr) => {
        if $cli_world.skip_assertions.get() {
            return;
        }
    };
}

/// Sets up a temporary directory and returns its path as a string.
fn setup_temp_dir(cli_world: &CliWorld) -> String {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let target_dir = temp_dir.path().to_string_lossy().to_string();
    cli_world._temp_dir.replace(Some(temp_dir));
    target_dir
}

/// Asserts that the CLI exit status matches the expected success state.
fn assert_exit_status(cli_world: &CliWorld, expected_success: bool) {
    skip_if_needed!(cli_world);

    let output = get_output(cli_world);
    if expected_success {
        assert!(
            output.status.success(),
            "expected success, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert!(
            !output.status.success(),
            "expected failure, stdout: {}, stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[given("the installer is invoked with dry-run and a target directory")]
fn given_dry_run_with_target_dir(cli_world: &CliWorld) {
    let Some(channel) = ensure_required_toolchain_available(cli_world) else {
        return;
    };

    let target_dir = setup_temp_dir(cli_world);

    cli_world.args.replace(vec![
        "--dry-run".to_owned(),
        "--toolchain".to_owned(),
        channel,
        "--target-dir".to_owned(),
        target_dir,
    ]);
}

#[given("the installer is invoked with dry-run and an unknown lint")]
fn given_dry_run_unknown_lint(cli_world: &CliWorld) {
    cli_world.args.replace(vec![
        "--dry-run".to_owned(),
        "--lint".to_owned(),
        "nonexistent_lint".to_owned(),
    ]);
}

#[given("the installer is invoked in suite-only mode to a temporary directory")]
fn given_suite_only_install(cli_world: &CliWorld) {
    let Some(_channel) = ensure_required_toolchain_available(cli_world) else {
        return;
    };

    let target_dir = setup_temp_dir(cli_world);

    cli_world.args.replace(vec![
        "--suite-only".to_owned(),
        "--jobs".to_owned(),
        "1".to_owned(),
        "--target-dir".to_owned(),
        target_dir,
    ]);
}

#[when("the installer CLI is run")]
fn when_installer_cli_run(cli_world: &CliWorld) {
    skip_if_needed!(cli_world);

    let args = cli_world.args.borrow();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_whitaker-install"));
    cmd.args(args.iter());
    cmd.current_dir(workspace_root());

    let output = cmd.output().expect("failed to run whitaker-install");
    cli_world.output.replace(Some(output));
}

/// Helper function to retrieve the command output from the CLI world.
fn get_output(cli_world: &CliWorld) -> std::cell::Ref<'_, Output> {
    let output = cli_world.output.borrow();
    std::cell::Ref::map(output, |opt| opt.as_ref().expect("output not set"))
}

#[then("the CLI exits successfully")]
fn then_cli_exits_successfully(cli_world: &CliWorld) {
    assert_exit_status(cli_world, true);
}

#[then("dry-run output is shown")]
fn then_dry_run_output_is_shown(cli_world: &CliWorld) {
    skip_if_needed!(cli_world);

    let toolchain = cli_world.toolchain.borrow();
    let toolchain = toolchain.as_ref().expect("toolchain not set");

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("Dry run - no files will be modified"));
    assert!(stderr.contains(&format!("Toolchain: {toolchain}")));
    assert!(stderr.contains("Crates to build:"));
    assert!(stderr.contains("module_max_lines"));
    assert!(stderr.contains("suite"));

    let temp_dir = cli_world._temp_dir.borrow();
    let temp_dir = temp_dir.as_ref().expect("temp dir not set");
    let target_dir = temp_dir.path().to_string_lossy();
    assert!(stderr.contains(&format!("Target directory: {target_dir}")));
}

#[then("the CLI exits with an error")]
fn then_cli_exits_with_error(cli_world: &CliWorld) {
    assert_exit_status(cli_world, false);
}

#[then("an unknown lint message is shown")]
fn then_unknown_lint_message_is_shown(cli_world: &CliWorld) {
    skip_if_needed!(cli_world);

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Lint validation fails before dry-run configuration output is rendered.
    assert!(
        !stderr.contains("Dry run - no files will be modified"),
        "dry-run configuration output should not be printed on unknown-lint error, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("Crates to build:"),
        "dry-run configuration output should not be printed on unknown-lint error, stderr: {stderr}"
    );

    assert!(
        stderr.contains("lint crate nonexistent_lint not found"),
        "unexpected stderr: {stderr}"
    );
}

#[then("installation succeeds or is skipped")]
fn then_installation_succeeds_or_is_skipped(cli_world: &CliWorld) {
    skip_if_needed!(cli_world);

    let output = get_output(cli_world);
    assert!(
        output.status.success(),
        "installation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[then("the suite library is staged")]
fn then_suite_library_is_staged(cli_world: &CliWorld) {
    skip_if_needed!(cli_world);

    let output = get_output(cli_world);
    let channel = cli_world.toolchain.borrow();
    let channel = channel.as_ref().expect("toolchain not set");
    let temp_dir = cli_world._temp_dir.borrow();
    let temp_dir = temp_dir.as_ref().expect("temp dir not set");

    let staging_dir = temp_dir.path().join(channel).join("release");
    let entries = std::fs::read_dir(&staging_dir)
        .expect("staging directory should exist")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let expected_substring = format!("suite@{channel}");
    let matches = entries
        .iter()
        .filter(|name| name.contains(&expected_substring))
        .count();

    assert!(
        matches == 1,
        "expected exactly one suite library matching '{expected_substring}' to be staged in {staging_dir:?}; matches={matches}, entries={entries:?}, stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

// Do not reorder scenarios in tests/features/installer.feature — bindings are
// index-based.
#[scenario(path = "tests/features/installer.feature", index = 12)]
fn scenario_dry_run_outputs_configuration(cli_world: CliWorld) {
    let _ = cli_world;
}

#[scenario(path = "tests/features/installer.feature", index = 13)]
fn scenario_dry_run_rejects_unknown_lint(cli_world: CliWorld) {
    let _ = cli_world;
}

#[scenario(path = "tests/features/installer.feature", index = 14)]
fn scenario_install_suite_to_temp_dir(cli_world: CliWorld) {
    let _ = cli_world;
}
