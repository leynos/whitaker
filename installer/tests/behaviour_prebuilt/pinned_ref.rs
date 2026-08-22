//! Pinned-ref BDD steps and scenarios for prebuilt installation.

use super::{DEFAULT_TOOLCHAIN, FAKE_ARCHIVE, ManifestBehaviour, PrebuiltWorld, world};
use rstest_bdd_macros::{given, scenario};
use whitaker_installer::test_utils::{prebuilt_manifest_json, sha256_hex};

#[given("the pinned commit does not match the manifest git SHA")]
fn given_pinned_commit_mismatch(world: &mut PrebuiltWorld) {
    world.expected_git_sha = Some("deadbeef00000000000000000000000000000000".to_owned());
}

#[given("the pinned commit matches the manifest git SHA")]
fn given_pinned_commit_matches(world: &mut PrebuiltWorld) {
    let commit = "abc12340000000000000000000000000000000ab";
    world.expected_git_sha = Some(commit.to_owned());
    world.manifest_behaviour = Some(ManifestBehaviour::Ok(
        prebuilt_manifest_json(
            DEFAULT_TOOLCHAIN,
            super::DEFAULT_TARGET,
            sha256_hex(FAKE_ARCHIVE),
        )
        .replacen("abc1234", commit, 1),
    ));
}

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Prebuilt is skipped when the pinned ref does not match"
)]
fn scenario_pinned_ref_mismatch(world: PrebuiltWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/prebuilt_download.feature",
    name = "Prebuilt succeeds when the pinned ref matches"
)]
fn scenario_pinned_ref_match(world: PrebuiltWorld) {
    let _ = world;
}
