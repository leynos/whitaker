//! Scenario bindings for dependency-binary installation behaviour tests.

use rstest_bdd_macros::scenario;

use super::{DependencyBinaryWorld, world};

// Do not reorder scenarios in `tests/features/dependency_binaries.feature`:
// bindings intentionally use the feature's stable indices.
#[scenario(path = "tests/features/dependency_binaries.feature", index = 0)]
fn scenario_install_cargo_dylint_from_repository(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 1)]
fn scenario_install_dylint_link_from_repository(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 2)]
fn scenario_repository_falls_back_to_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 3)]
fn scenario_repository_and_binstall_and_cargo_all_fail(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 4)]
fn scenario_repository_and_binstall_fall_back_to_cargo_install(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 5)]
fn scenario_repository_and_binstall_fall_back_to_failed_cargo_install(
    world: DependencyBinaryWorld,
) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 6)]
fn scenario_repository_verification_failure_uses_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 7)]
fn scenario_unsupported_target_uses_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 8)]
fn scenario_repository_success_without_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 9)]
fn scenario_provenance_lists_both_dependencies(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 10)]
fn scenario_dylint_link_missing_after_install_fails(world: DependencyBinaryWorld) {
    let _ = world;
}
