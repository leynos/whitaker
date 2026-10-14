//! Scenario bindings for prebuilt-download behaviour tests.

use rstest_bdd_macros::scenario;

use super::{PrebuiltWorld, world};

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Successful prebuilt download and verification"
)]
fn scenario_successful_download(world: PrebuiltWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Checksum mismatch triggers fallback"
)]
fn scenario_checksum_mismatch(world: PrebuiltWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Network failure triggers fallback"
)]
fn scenario_network_failure(world: PrebuiltWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Missing artefact triggers fallback"
)]
fn scenario_not_found(world: PrebuiltWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Destination path creation failure triggers fallback"
)]
fn scenario_destination_creation_failure(world: PrebuiltWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Toolchain mismatch triggers fallback"
)]
fn scenario_toolchain_mismatch(world: PrebuiltWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Build-only flag skips prebuilt"
)]
fn scenario_build_only(world: PrebuiltWorld) {
    let _ = world;
}
