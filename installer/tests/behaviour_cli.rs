//! End-to-end CLI behaviour tests for `whitaker-installer`.
//!
//! These scenarios invoke the installer binary and validate dry-run output,
//! error handling, and (when possible) installation results.

mod prebuilt_markers;
#[path = "behaviour_cli/scenarios.rs"]
mod scenarios;
#[path = "behaviour_cli/support.rs"]
mod support;

use rstest_bdd_macros::{given, then, when};
use std::process::Command;
pub(crate) use support::{CliWorld, cli_world};
use support::{
    assert_cli_exits_successfully, assert_cli_exits_with_error, assert_dry_run_output_is_shown,
    assert_installation_succeeds_or_is_skipped, assert_suite_library_is_staged,
    assert_unknown_lint_message_is_shown, configure_dry_run_unknown_lint,
    configure_dry_run_with_target_dir, configure_suite_install, is_toolchain_installed,
    pinned_toolchain_channel, run_installer_cli, workspace_root,
};

#[given("the installer is invoked with dry-run and a target directory")]
fn given_dry_run_with_target_dir(cli_world: &CliWorld) {
    configure_dry_run_with_target_dir(cli_world);
}

#[given("the installer is invoked with dry-run and an unknown lint")]
fn given_dry_run_unknown_lint(cli_world: &CliWorld) {
    configure_dry_run_unknown_lint(cli_world);
}

#[given("the installer is invoked to a temporary directory")]
fn given_suite_install(cli_world: &CliWorld) {
    configure_suite_install(cli_world);
}

#[when("the installer CLI is run")]
fn when_installer_cli_run(cli_world: &CliWorld) {
    run_installer_cli(cli_world);
}

#[then("the CLI exits successfully")]
fn then_cli_exits_successfully(cli_world: &CliWorld) {
    assert_cli_exits_successfully(cli_world);
}

#[then("dry-run output is shown")]
fn then_dry_run_output_is_shown(cli_world: &CliWorld) {
    assert_dry_run_output_is_shown(cli_world);
}

#[then("the CLI exits with an error")]
fn then_cli_exits_with_error(cli_world: &CliWorld) {
    assert_cli_exits_with_error(cli_world);
}

#[then("an unknown lint message is shown")]
fn then_unknown_lint_message_is_shown(cli_world: &CliWorld) {
    assert_unknown_lint_message_is_shown(cli_world);
}

#[then("installation succeeds or is skipped")]
fn then_installation_succeeds_or_is_skipped(cli_world: &CliWorld) {
    assert_installation_succeeds_or_is_skipped(cli_world);
}

#[then("the suite library is staged")]
fn then_suite_library_is_staged(cli_world: &CliWorld) {
    assert_suite_library_is_staged(cli_world);
}

#[test]
fn dry_run_reports_verbosity_levels() {
    let channel = pinned_toolchain_channel();
    if !is_toolchain_installed(&channel) {
        return;
    }

    let run_dry_run = |extra_args: &[&str]| -> String {
        let mut args = vec!["--dry-run", "--toolchain", channel.as_str()];
        args.extend_from_slice(extra_args);

        let output = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"))
            .args(args)
            .current_dir(workspace_root())
            .output()
            .expect("failed to run whitaker-installer");

        assert!(
            output.status.success(),
            "expected success, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8_lossy(&output.stderr).to_string()
    };

    let default_output = run_dry_run(&[]);
    assert!(default_output.contains("Verbosity level: 0"));

    let single_output = run_dry_run(&["-v"]);
    assert!(single_output.contains("Verbosity level: 1"));

    let double_output = run_dry_run(&["-vv"]);
    assert!(double_output.contains("Verbosity level: 2"));
}
