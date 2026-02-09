//! Behaviour-driven tests for artefact naming, manifest, and verification
//! policy.
//!
//! These scenarios validate the domain types defined in the `artefact` module
//! against the rules specified in ADR-001. Tests use the rstest-bdd v0.5.0
//! mutable world pattern.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_installer::artefact::error::ArtefactError;
use whitaker_installer::artefact::git_sha::GitSha;
use whitaker_installer::artefact::manifest::{
    GeneratedAt, Manifest, ManifestContent, ManifestProvenance,
};
use whitaker_installer::artefact::naming::ArtefactName;
use whitaker_installer::artefact::schema_version::SchemaVersion;
use whitaker_installer::artefact::sha256_digest::Sha256Digest;
use whitaker_installer::artefact::target::TargetTriple;
use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;
use whitaker_installer::artefact::verification::{VerificationFailureAction, VerificationPolicy};

// ---------------------------------------------------------------------------
// World types
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ArtefactWorld {
    git_sha: Option<GitSha>,
    toolchain: Option<ToolchainChannel>,
    target: Option<TargetTriple>,
    artefact_name: Option<ArtefactName>,
    target_error: Option<ArtefactError>,
    sha_error: Option<ArtefactError>,
    channel_error: Option<ArtefactError>,
    manifest: Option<Manifest>,
    policy: Option<VerificationPolicy>,
    failure_action: Option<VerificationFailureAction>,
    all_triples_ok: Option<bool>,
}

#[fixture]
fn world() -> ArtefactWorld {
    ArtefactWorld::default()
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("a git SHA \"{sha}\"")]
fn given_git_sha(world: &mut ArtefactWorld, sha: String) {
    world.git_sha = Some(GitSha::try_from(sha).expect("test SHA"));
}

#[given("a toolchain channel \"{channel}\"")]
fn given_toolchain_channel(world: &mut ArtefactWorld, channel: String) {
    world.toolchain = Some(ToolchainChannel::try_from(channel).expect("test channel"));
}

#[given("a target triple \"{triple}\"")]
fn given_target_triple(world: &mut ArtefactWorld, triple: String) {
    world.target = Some(TargetTriple::try_from(triple).expect("test triple"));
}

#[when("an artefact name is constructed")]
fn when_artefact_name_constructed(world: &mut ArtefactWorld) {
    let sha = world.git_sha.clone().expect("git_sha set");
    let ch = world.toolchain.clone().expect("toolchain set");
    let tgt = world.target.clone().expect("target set");
    world.artefact_name = Some(ArtefactName::new(sha, ch, tgt));
}

#[then("the filename is \"{expected}\"")]
fn then_filename_matches(world: &mut ArtefactWorld, expected: String) {
    let name = world.artefact_name.as_ref().expect("artefact_name set");
    assert_eq!(name.filename(), expected);
}

#[given("an invalid target triple \"{triple}\"")]
fn given_invalid_target(world: &mut ArtefactWorld, triple: String) {
    world.target_error = TargetTriple::try_from(triple).err();
}

#[then("the target triple is rejected")]
fn then_target_rejected(world: &mut ArtefactWorld) {
    assert!(
        world.target_error.is_some(),
        "expected target validation to fail"
    );
    assert!(matches!(
        world.target_error.as_ref().expect("checked above"),
        ArtefactError::UnsupportedTarget { .. }
    ));
}

#[given("all supported target triples")]
fn given_all_supported(world: &mut ArtefactWorld) {
    let all_ok = TargetTriple::supported()
        .iter()
        .all(|t| TargetTriple::try_from(*t).is_ok());
    world.all_triples_ok = Some(all_ok);
}

#[then("every triple is accepted")]
fn then_all_accepted(world: &mut ArtefactWorld) {
    assert_eq!(world.all_triples_ok, Some(true));
}

#[given("an invalid git SHA \"{sha}\"")]
fn given_invalid_sha(world: &mut ArtefactWorld, sha: String) {
    world.sha_error = GitSha::try_from(sha).err();
}

#[then("the git SHA is rejected")]
fn then_sha_rejected(world: &mut ArtefactWorld) {
    assert!(world.sha_error.is_some(), "expected SHA validation to fail");
    assert!(matches!(
        world.sha_error.as_ref().expect("checked above"),
        ArtefactError::InvalidGitSha { .. }
    ));
}

#[given("an empty toolchain channel")]
fn given_empty_channel(world: &mut ArtefactWorld) {
    world.channel_error = ToolchainChannel::try_from("").err();
}

#[then("the toolchain channel is rejected")]
fn then_channel_rejected(world: &mut ArtefactWorld) {
    assert!(
        world.channel_error.is_some(),
        "expected channel validation to fail"
    );
    assert!(matches!(
        world.channel_error.as_ref().expect("checked above"),
        ArtefactError::InvalidToolchainChannel { .. }
    ));
}

#[given("a complete set of manifest fields")]
fn given_manifest_fields(world: &mut ArtefactWorld) {
    world.git_sha = Some(GitSha::try_from("abc1234").expect("valid sha"));
    world.toolchain =
        Some(ToolchainChannel::try_from("nightly-2025-09-18").expect("valid channel"));
    world.target = Some(TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target"));
}

#[when("a manifest is constructed")]
fn when_manifest_constructed(world: &mut ArtefactWorld) {
    let provenance = ManifestProvenance {
        git_sha: world.git_sha.clone().expect("git_sha set"),
        schema_version: SchemaVersion::current(),
        toolchain: world.toolchain.clone().expect("toolchain set"),
        target: world.target.clone().expect("target set"),
    };
    let digest_hex = "a".repeat(64);
    let content = ManifestContent {
        generated_at: GeneratedAt::new("2026-02-03T00:00:00Z"),
        files: vec!["libwhitaker_lints.so".to_owned()],
        sha256: Sha256Digest::try_from(digest_hex.as_str()).expect("valid digest"),
    };
    world.manifest = Some(Manifest::new(provenance, content));
}

#[then("all manifest fields are accessible")]
fn then_manifest_accessible(world: &mut ArtefactWorld) {
    let m = world.manifest.as_ref().expect("manifest set");
    assert_eq!(m.git_sha().as_str(), "abc1234");
    assert_eq!(m.schema_version().as_u32(), 1);
    assert_eq!(m.toolchain().as_str(), "nightly-2025-09-18");
    assert_eq!(m.target().as_str(), "x86_64-unknown-linux-gnu");
    assert_eq!(m.generated_at().as_str(), "2026-02-03T00:00:00Z");
    assert_eq!(m.files().len(), 1);
    assert_eq!(m.sha256().as_str().len(), 64);
}

#[given("the default verification policy")]
fn given_default_policy(world: &mut ArtefactWorld) {
    world.policy = Some(VerificationPolicy::default());
}

#[then("checksum verification is required")]
fn then_checksum_required(world: &mut ArtefactWorld) {
    let policy = world.policy.as_ref().expect("policy set");
    assert!(policy.require_checksum());
}

#[given("the default failure action")]
fn given_default_failure_action(world: &mut ArtefactWorld) {
    world.failure_action = Some(VerificationFailureAction::default());
}

#[then("the action is fallback with warning")]
fn then_action_is_fallback(world: &mut ArtefactWorld) {
    assert_eq!(
        world.failure_action,
        Some(VerificationFailureAction::FallbackWithWarning)
    );
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Construct artefact name from valid components"
)]
fn scenario_construct_artefact_name(world: ArtefactWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Reject unsupported target triple"
)]
fn scenario_reject_unsupported_target(world: ArtefactWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Accept all five supported target triples"
)]
fn scenario_accept_all_supported_targets(world: ArtefactWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Reject invalid git SHA"
)]
fn scenario_reject_invalid_git_sha(world: ArtefactWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Reject empty toolchain channel"
)]
fn scenario_reject_empty_channel(world: ArtefactWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Construct manifest with all fields"
)]
fn scenario_construct_manifest(world: ArtefactWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Default verification policy requires checksum"
)]
fn scenario_default_verification_policy(world: ArtefactWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Verification failure triggers fallback"
)]
fn scenario_verification_failure_fallback(world: ArtefactWorld) {
    let _ = world;
}
