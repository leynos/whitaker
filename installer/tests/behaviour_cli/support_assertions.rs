//! Assertions over the installer CLI's observed output for behaviour tests.

use std::path::PathBuf;

use super::{CliWorld, expected_prebuilt_target_dir, get_output, matching_files};
use crate::prebuilt_markers::PREBUILT_INSTALL_MARKER;

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

pub(crate) fn assert_cli_exits_successfully(cli_world: &CliWorld) {
    assert_exit_status(cli_world, true);
}

pub(crate) fn assert_dry_run_output_is_shown(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let toolchain_slot = cli_world.toolchain.borrow();
    let Some(toolchain) = toolchain_slot.as_ref() else {
        panic!("toolchain not set");
    };

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("Dry run - no files will be modified"),
        "expected dry-run banner in stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("Toolchain: {toolchain}")),
        "expected toolchain line in stderr: {stderr}"
    );
    assert!(
        stderr.contains("Crates to build:"),
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

    let temp_dir_slot = cli_world.temp_dir.borrow();
    let Some(temp_dir) = temp_dir_slot.as_ref() else {
        panic!("temp dir not set");
    };
    let target_dir = temp_dir.path().to_string_lossy();
    let expected_target_dir =
        expected_prebuilt_target_dir(toolchain).unwrap_or_else(|| target_dir.into_owned());
    assert!(
        stderr.contains(&format!("Target directory: {expected_target_dir}")),
        "expected target directory line in stderr: {stderr}"
    );
}

pub(crate) fn assert_cli_exits_with_error(cli_world: &CliWorld) {
    assert_exit_status(cli_world, false);
}

pub(crate) fn assert_unknown_lint_message_is_shown(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("Dry run - no files will be modified"),
        "dry-run configuration output should not be printed on unknown-lint error, stderr: \
         {stderr}"
    );
    assert!(
        !stderr.contains("Crates to build:"),
        "dry-run configuration output should not be printed on unknown-lint error, stderr: \
         {stderr}"
    );
    assert!(
        stderr.contains("lint crate nonexistent_lint not found"),
        "unexpected stderr: {stderr}"
    );
}

pub(crate) fn assert_experimental_lint_opt_in_message_is_shown(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("Dry run - no files will be modified"),
        "dry-run configuration output should not be printed on experimental-lint error, stderr: \
         {stderr}"
    );
    assert!(
        !stderr.contains("Crates to build:"),
        "dry-run configuration output should not be printed on experimental-lint error, stderr: \
         {stderr}"
    );
    assert!(
        stderr.contains(
            "experimental lint crate rstest_helper_should_be_fixture requires --experimental"
        ),
        "unexpected stderr: {stderr}"
    );
}

pub(crate) fn assert_experimental_lint_dry_run_output_is_shown(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("Dry run - no files will be modified"),
        "expected dry-run banner in stderr: {stderr}"
    );
    assert!(
        stderr.contains("Crates to build:"),
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

    let output = get_output(cli_world);
    assert!(
        output.status.success(),
        "installation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn assert_suite_library_is_staged(cli_world: &CliWorld) {
    if cli_world.skip_assertions.get() {
        return;
    }

    let output = get_output(cli_world);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let channel_slot = cli_world.toolchain.borrow();
    let Some(channel) = channel_slot.as_ref() else {
        panic!("toolchain not set");
    };
    let needle = format!("whitaker_suite@{channel}");

    if stderr.contains(PREBUILT_INSTALL_MARKER)
        && let Some(dir) = expected_prebuilt_target_dir(channel)
    {
        let prebuilt_path = PathBuf::from(&dir);
        let matches = matching_files(&prebuilt_path, &needle);
        assert!(
            !matches.is_empty(),
            "prebuilt marker found in stderr but no library matching '{needle}' in {}, \
             entries={:?}",
            prebuilt_path.display(),
            matching_files(&prebuilt_path, ""),
        );
        return;
    }

    let temp_dir_slot = cli_world.temp_dir.borrow();
    let Some(temp_dir) = temp_dir_slot.as_ref() else {
        panic!("temp dir not set");
    };
    let staging_dir = temp_dir.path().join(channel).join("release");
    let matches = matching_files(&staging_dir, &needle);

    assert!(
        matches.len() == 1,
        "expected exactly one suite library matching '{needle}' in {}, matches={matches:?}, \
         entries={:?}, stdout={}, stderr={stderr}",
        staging_dir.display(),
        matching_files(&staging_dir, ""),
        String::from_utf8_lossy(&output.stdout),
    );
}
