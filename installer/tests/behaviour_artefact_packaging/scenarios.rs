//! Scenario bindings for artefact packaging behaviour tests.

use rstest_bdd_macros::scenario;

use super::{PackagingWorld, world};

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Package a single library file into a tar.zst archive"
)]
fn scenario_package_single_library(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Manifest JSON contains all required fields"
)]
fn scenario_manifest_fields(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Manifest sha256 matches the archive digest"
)]
fn scenario_manifest_digest_self_consistency(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Archive SHA-256 is a valid digest"
)]
fn scenario_archive_sha256(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Packaging rejects an empty file list"
)]
fn scenario_reject_empty_files(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Archive filename matches ArtefactName convention"
)]
fn scenario_filename_matches(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Archive contains multiple library files"
)]
fn scenario_multi_library(world: PackagingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_packaging.feature",
    name = "Manifest files field lists all library basenames"
)]
fn scenario_manifest_files_field(world: PackagingWorld) {
    let _ = world;
}
