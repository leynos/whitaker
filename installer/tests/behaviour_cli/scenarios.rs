//! Scenario bindings for CLI behaviour tests.

use super::{CliWorld, cli_world};
use rstest_bdd_macros::scenario;

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
