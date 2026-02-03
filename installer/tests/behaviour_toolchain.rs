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
mod toolchain_steps;

use rstest_bdd_macros::scenario;
use toolchain_steps::{ToolchainWorld, world};

// Import step definitions so rstest-bdd's scenario macro can discover them.
// These imports appear unused to clippy because they're consumed by macro
// expansion, not direct source-level calls.
#[allow(unused_imports)]
use toolchain_steps::{
    given_auto_detect_toolchain, given_auto_detect_toolchain_install,
    given_auto_detect_toolchain_quiet, given_isolated_rustup_auto_install,
    given_isolated_rustup_quiet, given_nonexistent_toolchain, given_nonexistent_toolchain_quiet,
    then_cli_exits_successfully, then_cli_exits_with_error, then_dry_run_shows_toolchain,
    then_error_includes_toolchain_name, then_error_mentions_install_failure,
    then_error_output_is_minimal, then_install_message_shown,
    then_installation_succeeds_or_is_skipped, then_no_install_message,
    then_suite_library_is_staged, then_toolchain_installed_in_isolated_env, when_installer_cli_run,
};

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
