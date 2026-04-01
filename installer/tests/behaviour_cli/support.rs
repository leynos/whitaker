//! Shared fixtures, command helpers, and assertions for CLI behaviour tests.

use super::prebuilt_markers::PREBUILT_INSTALL_MARKER;
use rstest::fixture;
use std::cell::{Cell, Ref, RefCell};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::dirs::SystemBaseDirs;
use whitaker_installer::prebuilt_path::prebuilt_library_dir;
use whitaker_installer::test_support::TEST_STAGE_SUITE_ENV;
use whitaker_installer::toolchain::parse_toolchain_channel;

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

#[fixture]
pub(super) fn cli_world() -> CliWorld {
    CliWorld::default()
}

pub(super) fn workspace_root() -> PathBuf {
    PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest dir should have parent")
        .to_owned()
}

pub(super) fn pinned_toolchain_channel() -> String {
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

pub(super) fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn skip_scenario_when_toolchain_missing(cli_world: &CliWorld, channel: &str) {
    if !is_toolchain_installed(channel) {
        eprintln!(
            "Skipping scenario because rustup toolchain '{channel}' is not installed. Install this toolchain to run these tests."
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
    (!cli_world.skip_assertions.get()).then_some(channel)
}

pub(super) fn ensure_required_toolchain_available(cli_world: &CliWorld) -> Option<String> {
    cli_world.requires_toolchain.set(true);
    ensure_toolchain_available(cli_world)
}

pub(super) fn setup_temp_dir(cli_world: &CliWorld) -> String {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
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
        .map(|path| path.into_string())
}

fn matching_files(dir: &Path, substring: &str) -> Vec<String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => panic!("failed to read directory {}: {error}", dir.display()),
    };
    entries
        .map(|entry| match entry {
            Ok(entry) => entry.file_name().to_string_lossy().to_string(),
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

    let output = command.output().expect("failed to run whitaker-installer");
    cli_world.output.replace(Some(output));
}

pub(super) fn get_output(cli_world: &CliWorld) -> Ref<'_, Output> {
    let output = cli_world.output.borrow();
    Ref::map(output, |opt| opt.as_ref().expect("output not set"))
}

fn assert_exit_status(cli_world: &CliWorld, expected_success: bool) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.success(),
        expected_success,
        "expected success={expected_success}, stdout={}, stderr={stderr}",
        String::from_utf8_lossy(&output.stdout),
    );
}

pub(super) fn assert_cli_exits_successfully(cli_world: &CliWorld) {
    assert_exit_status(cli_world, true);
}

pub(super) fn assert_dry_run_output_is_shown(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let toolchain = cli_world.toolchain.borrow();
    let toolchain = toolchain.as_ref().expect("toolchain not set");

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("Dry run - no files will be modified"));
    assert!(stderr.contains(&format!("Toolchain: {toolchain}")));
    assert!(stderr.contains("Crates to build:"));
    assert!(stderr.contains("whitaker_suite"));
    assert!(
        !stderr.contains("module_max_lines"),
        "individual lint crate should not appear in suite-only mode, stderr: {stderr}"
    );

    let temp_dir = cli_world.temp_dir.borrow();
    let temp_dir = temp_dir.as_ref().expect("temp dir not set");
    let target_dir = temp_dir.path().to_string_lossy();
    let expected_target_dir =
        expected_prebuilt_target_dir(toolchain).unwrap_or_else(|| target_dir.into_owned());
    assert!(stderr.contains(&format!("Target directory: {expected_target_dir}")));
}

pub(super) fn assert_cli_exits_with_error(cli_world: &CliWorld) {
    assert_exit_status(cli_world, false);
}

pub(super) fn assert_unknown_lint_message_is_shown(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);

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

pub(super) fn assert_installation_succeeds_or_is_skipped(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = get_output(cli_world);
    assert!(
        output.status.success(),
        "installation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn assert_suite_library_is_staged(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let channel = cli_world.toolchain.borrow();
    let channel = channel.as_ref().expect("toolchain not set");
    let needle = format!("whitaker_suite@{channel}");

    if stderr.contains(PREBUILT_INSTALL_MARKER)
        && let Some(dir) = expected_prebuilt_target_dir(channel)
    {
        let prebuilt_path = PathBuf::from(&dir);
        let matches = matching_files(&prebuilt_path, &needle);
        assert!(
            !matches.is_empty(),
            "prebuilt marker found in stderr but no library matching \
             '{needle}' in {prebuilt_path:?}, entries={:?}",
            matching_files(&prebuilt_path, ""),
        );
        return;
    }

    let temp_dir = cli_world.temp_dir.borrow();
    let temp_dir = temp_dir.as_ref().expect("temp dir not set");
    let staging_dir = temp_dir.path().join(channel).join("release");
    let matches = matching_files(&staging_dir, &needle);

    assert!(
        matches.len() == 1,
        "expected exactly one suite library matching '{needle}' in \
         {staging_dir:?}, matches={matches:?}, entries={:?}, \
         stdout={}, stderr={stderr}",
        matching_files(&staging_dir, ""),
        String::from_utf8_lossy(&output.stdout),
    );
}
