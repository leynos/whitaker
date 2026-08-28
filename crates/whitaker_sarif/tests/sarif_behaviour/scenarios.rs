//! Scenario bindings for SARIF behaviour tests.

use rstest_bdd_macros::scenario;

use super::{SarifWorld, world};

// Do not reorder scenarios in `tests/features/sarif.feature`: bindings
// intentionally use the feature's stable indices.
#[scenario(path = "tests/features/sarif.feature", index = 0)]
fn scenario_minimal_log(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 1)]
fn scenario_result_with_rule(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 2)]
fn scenario_whitaker_properties(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 3)]
fn scenario_merge_deduplicates(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 4)]
fn scenario_round_trip(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 5)]
fn scenario_empty_log(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 6)]
fn scenario_all_rules(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 7)]
fn scenario_path_helpers(world: SarifWorld) {
    let _ = world;
}
