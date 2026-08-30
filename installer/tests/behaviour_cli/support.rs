//! Shared fixtures and command helpers for CLI behaviour tests.

use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    process::{Command, Output},
};

use rstest::fixture;
use tempfile::TempDir;
use whitaker_installer::{
    dirs::SystemBaseDirs,
    prebuilt_path::prebuilt_library_dir,
    test_support::TEST_STAGE_SUITE_ENV,
    toolchain::parse_toolchain_channel,
};

#[derive(Default)]
pub(super) struct CliWorld {
    args: RefCell<Vec<String>>,
    output: RefCell<Option<Output>>,
    skip_assertions: Cell<bool>,
    requires_toolchain: Cell<bool>,
    should_use_test_staged_suite: Cell<bool>,
    toolchain: RefCell<Option<String>>,
    // Keep temp_dir alive for the lifetime of the scenario.
    temp_dir: RefCell<Option<TempDir>>,
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
pub(super) fn cli_world() -> CliWorld { CliWorld::default() }

pub(super) fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));
    let Some(parent) = manifest_dir.parent() else {
        panic!("manifest dir should have parent");
    };
    parent.to_owned()
}

pub(super) fn pinned_toolchain_channel() -> String {
    let toolchain_path = workspace_root().join("rust-toolchain.toml");
    let Ok(contents) = std::fs::read_to_string(&toolchain_path) else {
        panic!(
            "rust-toolchain.toml at {} should be readable",
            toolchain_path.display()
        );
    };
    let Ok(channel) = parse_toolchain_channel(&contents) else {
        panic!(
            "rust-toolchain.toml at {} should declare a channel",
            toolchain_path.display()
        );
    };
    channel
}

pub(super) fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn skip_scenario_when_toolchain_missing(cli_world: &CliWorld, channel: &str) {
    if !is_toolchain_installed(channel) {
        cli_world.skip_assertions.set(true);
        rstest_bdd::skip!(
            "rustup toolchain '{channel}' is not installed. Install this toolchain to run these \
             tests.",
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
    (!cli_world.skip_assertions.get()).then_some(channel)
}

pub(super) fn ensure_required_toolchain_available(cli_world: &CliWorld) -> Option<String> {
    cli_world.requires_toolchain.set(true);
    ensure_toolchain_available(cli_world)
}

pub(super) fn setup_temp_dir(cli_world: &CliWorld) -> String {
    let Ok(temp_dir) = TempDir::new() else {
        panic!("temporary directory should be created");
    };
    let target_dir = temp_dir.path().to_string_lossy().to_string();
    cli_world.temp_dir.replace(Some(temp_dir));
    target_dir
}

fn detect_host_target() -> Option<String> {
    let output = Command::new("rustc").args(["-vV"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout.lines().find_map(|line| {
        line.strip_prefix("host: ")
            .map(str::trim)
            .map(ToOwned::to_owned)
    })
}

fn expected_prebuilt_target_dir(toolchain: &str) -> Option<String> {
    let dirs = SystemBaseDirs::new()?;
    let host_target = detect_host_target()?;
    prebuilt_library_dir(&dirs, toolchain, &host_target)
        .ok()
        .map(camino::Utf8PathBuf::into_string)
}

fn matching_files(dir: &Path, substring: &str) -> Vec<String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => panic!("failed to read directory {}: {error}", dir.display()),
    };
    entries
        .map(|entry| match entry {
            Ok(dir_entry) => dir_entry.file_name().to_string_lossy().to_string(),
            Err(error) => panic!("failed to read entry in {}: {error}", dir.display()),
        })
        .filter(|name| name.contains(substring))
        .collect()
}

pub(super) fn configure_dry_run_with_target_dir(cli_world: &CliWorld) {
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

pub(super) fn configure_dry_run_unknown_lint(cli_world: &CliWorld) {
    cli_world.args.replace(vec![
        "--dry-run".to_owned(),
        "--lint".to_owned(),
        "nonexistent_lint".to_owned(),
    ]);
}

pub(super) fn configure_dry_run_experimental_lint(cli_world: &CliWorld) {
    cli_world.args.replace(vec![
        "--dry-run".to_owned(),
        "--lint".to_owned(),
        "rstest_helper_should_be_fixture".to_owned(),
    ]);
}

pub(super) fn configure_dry_run_experimental_lint_with_opt_in(cli_world: &CliWorld) {
    let Some(channel) = ensure_required_toolchain_available(cli_world) else {
        return;
    };

    let target_dir = setup_temp_dir(cli_world);
    cli_world.args.replace(vec![
        "--dry-run".to_owned(),
        "--experimental".to_owned(),
        "--toolchain".to_owned(),
        channel,
        "--target-dir".to_owned(),
        target_dir,
        "--lint".to_owned(),
        "rstest_helper_should_be_fixture".to_owned(),
    ]);
}

pub(super) fn configure_suite_install(cli_world: &CliWorld) {
    let Some(_channel) = ensure_required_toolchain_available(cli_world) else {
        return;
    };

    let target_dir = setup_temp_dir(cli_world);
    cli_world.should_use_test_staged_suite.set(true);
    cli_world.args.replace(vec![
        "--target-dir".to_owned(),
        target_dir,
        "--skip-wrapper".to_owned(),
        "--skip-deps".to_owned(),
    ]);
}

pub(super) fn run_installer_cli(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let args = cli_world.args.borrow();
    let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
    command.args(args.iter());
    command.current_dir(workspace_root());
    if cli_world.should_use_test_staged_suite.get() {
        command.env(TEST_STAGE_SUITE_ENV, "1");
    }

    let Ok(output) = command.output() else {
        panic!("whitaker-installer should run");
    };
    cli_world.output.replace(Some(output));
}

#[path = "support_assertions.rs"]
mod assertions;

pub(super) use assertions::{
    assert_cli_exits_successfully,
    assert_cli_exits_with_error,
    assert_dry_run_output_is_shown,
    assert_experimental_lint_dry_run_output_is_shown,
    assert_experimental_lint_opt_in_message_is_shown,
    assert_installation_succeeds_or_is_skipped,
    assert_suite_library_is_staged,
    assert_unknown_lint_message_is_shown,
};
