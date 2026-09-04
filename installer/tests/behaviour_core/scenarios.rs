//! Scenario bindings for core installer behaviour tests.

use rstest_bdd_macros::scenario;

use super::{
    CrateResolutionWorld, SnippetWorld, ToolchainWorld, ValidationWorld, crate_world,
    snippet_world, toolchain_world, validation_world,
};

// Do not reorder scenarios in `tests/features/installer.feature`: bindings
// intentionally use the feature's stable indices.
#[scenario(path = "tests/features/installer.feature", index = 0)]
fn scenario_resolve_suite_only_by_default(crate_world: CrateResolutionWorld) {
    let _ = crate_world;
}

#[scenario(path = "tests/features/installer.feature", index = 1)]
fn scenario_resolve_individual_lints(crate_world: CrateResolutionWorld) {
    let _ = crate_world;
}

#[scenario(path = "tests/features/installer.feature", index = 2)]
fn scenario_resolve_individual_lints_with_experimental(crate_world: CrateResolutionWorld) {
    let _ = crate_world;
}

#[scenario(path = "tests/features/installer.feature", index = 3)]
fn scenario_resolve_specific_lints(crate_world: CrateResolutionWorld) {
    let _ = crate_world;
}

#[scenario(path = "tests/features/installer.feature", index = 4)]
fn scenario_validate_known_names(validation_world: ValidationWorld) {
    let _ = validation_world;
}

#[scenario(path = "tests/features/installer.feature", index = 5)]
fn scenario_reject_unknown_names(validation_world: ValidationWorld) {
    let _ = validation_world;
}

#[scenario(path = "tests/features/installer.feature", index = 6)]
fn scenario_parse_standard_toolchain(toolchain_world: ToolchainWorld) {
    let _ = toolchain_world;
}

#[scenario(path = "tests/features/installer.feature", index = 7)]
fn scenario_parse_top_level_channel(toolchain_world: ToolchainWorld) {
    let _ = toolchain_world;
}

#[scenario(path = "tests/features/installer.feature", index = 8)]
fn scenario_reject_missing_channel(toolchain_world: ToolchainWorld) {
    let _ = toolchain_world;
}

#[scenario(path = "tests/features/installer.feature", index = 9)]
fn scenario_generate_shell_snippets(snippet_world: SnippetWorld) {
    let _ = snippet_world;
}

#[scenario(path = "tests/features/installer.feature", index = 19)]
fn scenario_validate_experimental_names_with_opt_in(validation_world: ValidationWorld) {
    let _ = validation_world;
}

#[scenario(path = "tests/features/installer.feature", index = 20)]
fn scenario_reject_experimental_names_without_opt_in(validation_world: ValidationWorld) {
    let _ = validation_world;
}
