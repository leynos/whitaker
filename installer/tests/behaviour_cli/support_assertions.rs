//! Assertions over the installer CLI's observed output for behaviour tests.
//!
//! Helpers are layered following the toolchain step conventions: fallible
//! lookups return `Result` with a diagnostic message, queries are pure, and
//! the public assertion wrappers stay thin.

use std::path::{Path, PathBuf};

use super::{CliWorld, expected_prebuilt_target_dir, matching_files};
use crate::prebuilt_markers::PREBUILT_INSTALL_MARKER;

/// Banner printed when the installer runs in dry-run mode.
const DRY_RUN_BANNER: &str = "Dry run - no files will be modified";

/// Header preceding the crate list in dry-run output.
const CRATE_LIST_MARKER: &str = "Crates to build:";

/// Owned snapshot of the captured CLI process output.
struct CapturedOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

/// Snapshots the captured CLI output, or reports that the CLI has not run.
fn captured_output(cli_world: &CliWorld) -> Result<CapturedOutput, String> {
    let output_slot = cli_world.output.borrow();
    let output = output_slot
        .as_ref()
        .ok_or_else(|| "output not set; was the installer CLI run?".to_owned())?;
    Ok(CapturedOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Clones the configured toolchain channel, or reports that none is set.
fn configured_toolchain(cli_world: &CliWorld) -> Result<String, String> {
    cli_world
        .toolchain
        .borrow()
        .clone()
        .ok_or_else(|| "toolchain not set; was the scenario configured?".to_owned())
}

/// Clones the temporary target directory path, or reports that none is set.
fn temp_target_path(cli_world: &CliWorld) -> Result<PathBuf, String> {
    cli_world
        .temp_dir
        .borrow()
        .as_ref()
        .map(|temp_dir| temp_dir.path().to_owned())
        .ok_or_else(|| "temp dir not set; was the scenario configured?".to_owned())
}

/// Reports whether the stderr contains dry-run configuration output.
fn contains_dry_run_configuration(stderr: &str) -> bool {
    stderr.contains(DRY_RUN_BANNER) || stderr.contains(CRATE_LIST_MARKER)
}

/// Resolves the target directory the dry-run summary should report.
fn expected_dry_run_target_dir(toolchain: &str, fallback: &Path) -> String {
    expected_prebuilt_target_dir(toolchain)
        .unwrap_or_else(|| fallback.to_string_lossy().into_owned())
}

fn assert_exit_status(cli_world: &CliWorld, expected_success: bool) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = match captured_output(cli_world) {
        Ok(output) => output,
        Err(message) => panic!("{message}"),
    };
    assert_eq!(
        output.success, expected_success,
        "expected success={expected_success}, stdout={}, stderr={}",
        output.stdout, output.stderr,
    );
}

fn assert_error_output_is_shown(cli_world: &CliWorld, error_kind: &str, expected_error: &str) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = match captured_output(cli_world) {
        Ok(output) => output,
        Err(message) => panic!("{message}"),
    };
    let stderr = output.stderr;

    assert!(
        !contains_dry_run_configuration(&stderr),
        "dry-run configuration output should not be printed on {error_kind} error, stderr: \
         {stderr}"
    );
    assert!(
        stderr.contains(expected_error),
        "unexpected stderr: {stderr}"
    );
}

pub(crate) fn assert_cli_exits_successfully(cli_world: &CliWorld) {
    assert_exit_status(cli_world, true);
}

pub(crate) fn assert_dry_run_output_is_shown(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let toolchain = match configured_toolchain(cli_world) {
        Ok(toolchain) => toolchain,
        Err(message) => panic!("{message}"),
    };
    let output = match captured_output(cli_world) {
        Ok(output) => output,
        Err(message) => panic!("{message}"),
    };
    let stderr = output.stderr;

    assert!(
        stderr.contains(DRY_RUN_BANNER),
        "expected dry-run banner in stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("Toolchain: {toolchain}")),
        "expected toolchain line in stderr: {stderr}"
    );
    assert!(
        stderr.contains(CRATE_LIST_MARKER),
        "expected crate list in stderr: {stderr}"
    );
    assert!(
        stderr.contains("whitaker_suite"),
        "expected whitaker_suite in stderr: {stderr}"
    );
    assert!(
        !stderr.contains("module_max_lines"),
        "individual lint crate should not appear in suite-only mode, stderr: {stderr}"
    );

    let target_dir = match temp_target_path(cli_world) {
        Ok(path) => path,
        Err(message) => panic!("{message}"),
    };
    let expected_target_dir = expected_dry_run_target_dir(&toolchain, &target_dir);
    assert!(
        stderr.contains(&format!("Target directory: {expected_target_dir}")),
        "expected target directory line in stderr: {stderr}"
    );
}

pub(crate) fn assert_cli_exits_with_error(cli_world: &CliWorld) {
    assert_exit_status(cli_world, false);
}

pub(crate) fn assert_unknown_lint_message_is_shown(cli_world: &CliWorld) {
    assert_error_output_is_shown(
        cli_world,
        "unknown-lint",
        "lint crate nonexistent_lint not found",
    );
}

pub(crate) fn assert_experimental_lint_opt_in_message_is_shown(cli_world: &CliWorld) {
    assert_error_output_is_shown(
        cli_world,
        "experimental-lint",
        "experimental lint crate rstest_helper_should_be_fixture requires --experimental",
    );
}

pub(crate) fn assert_experimental_lint_dry_run_output_is_shown(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = match captured_output(cli_world) {
        Ok(output) => output,
        Err(message) => panic!("{message}"),
    };
    let stderr = output.stderr;

    assert!(
        stderr.contains(DRY_RUN_BANNER),
        "expected dry-run banner in stderr: {stderr}"
    );
    assert!(
        stderr.contains(CRATE_LIST_MARKER),
        "expected crate list in stderr: {stderr}"
    );
    assert!(
        stderr.contains("rstest_helper_should_be_fixture"),
        "expected experimental lint crate in stderr: {stderr}"
    );
    assert!(
        !stderr.contains(
            "experimental lint crate rstest_helper_should_be_fixture requires --experimental"
        ),
        "experimental opt-in error should not be printed when --experimental is set, stderr: \
         {stderr}"
    );
}

pub(crate) fn assert_installation_succeeds_or_is_skipped(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = match captured_output(cli_world) {
        Ok(output) => output,
        Err(message) => panic!("{message}"),
    };
    assert!(output.success, "installation failed: {}", output.stderr);
}

/// Asserts the prebuilt directory holds at least one library matching `needle`.
fn assert_prebuilt_library_present(prebuilt_path: &Path, needle: &str) {
    let matches = matching_files(prebuilt_path, needle);
    assert!(
        !matches.is_empty(),
        "prebuilt marker found in stderr but no library matching '{needle}' in {}, entries={:?}",
        prebuilt_path.display(),
        matching_files(prebuilt_path, ""),
    );
}

/// Asserts the staging directory holds exactly one library matching `needle`.
fn assert_staged_library_unique(staging_dir: &Path, needle: &str, output: &CapturedOutput) {
    let matches = matching_files(staging_dir, needle);
    assert!(
        matches.len() == 1,
        "expected exactly one suite library matching '{needle}' in {}, matches={matches:?}, \
         entries={:?}, stdout={}, stderr={}",
        staging_dir.display(),
        matching_files(staging_dir, ""),
        output.stdout,
        output.stderr,
    );
}

pub(crate) fn assert_suite_library_is_staged(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let channel = match configured_toolchain(cli_world) {
        Ok(channel) => channel,
        Err(message) => panic!("{message}"),
    };
    let output = match captured_output(cli_world) {
        Ok(output) => output,
        Err(message) => panic!("{message}"),
    };
    let needle = format!("whitaker_suite@{channel}");

    if output.stderr.contains(PREBUILT_INSTALL_MARKER)
        && let Some(dir) = expected_prebuilt_target_dir(&channel)
    {
        assert_prebuilt_library_present(&PathBuf::from(dir), &needle);
        return;
    }

    let target_dir = match temp_target_path(cli_world) {
        Ok(path) => path,
        Err(message) => panic!("{message}"),
    };
    let staging_dir = target_dir.join(&channel).join("release");
    assert_staged_library_unique(&staging_dir, &needle, &output);
}
