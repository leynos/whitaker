//! Pinned-ref BDD steps and scenarios for prebuilt installation.

use super::{PrebuiltWorld, world};
use rstest_bdd_macros::{given, scenario};

#[given("the pinned commit does not match the manifest git SHA")]
fn given_pinned_commit_mismatch(world: &mut PrebuiltWorld) {
    world.expected_git_sha = Some("deadbeef00000000000000000000000000000000".to_owned());
}

#[given("the pinned commit matches the manifest git SHA")]
fn given_pinned_commit_matches(world: &mut PrebuiltWorld) {
    world.expected_git_sha = Some("abc12340000000000000000000000000000000ab".to_owned());
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
