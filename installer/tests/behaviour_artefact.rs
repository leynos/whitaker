//! Behaviour-driven tests for artefact naming, manifest, and verification
//! policy.
//!
//! These scenarios validate the domain types defined in the `artefact` module
//! against the rules specified in ADR-001. Tests use the rstest-bdd v0.5.0
//! mutable world pattern with fallible steps.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_installer::artefact::{
    error::ArtefactError,
    git_sha::GitSha,
    manifest::{GeneratedAt, Manifest, ManifestContent, ManifestProvenance},
    naming::ArtefactName,
    schema_version::SchemaVersion,
    sha256_digest::Sha256Digest,
    target::TargetTriple,
    toolchain_channel::ToolchainChannel,
    verification::{VerificationFailureAction, VerificationPolicy},
};

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

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> ArtefactWorld { ArtefactWorld::default() }

/// Check that an error option contains a specific `ArtefactError` variant.
fn ensure_error_matches<F>(
    error: Option<&ArtefactError>,
    field_name: &str,
    predicate: F,
) -> Result<(), String>
where
    F: FnOnce(&ArtefactError) -> bool,
{
    let observed = error.ok_or_else(|| format!("expected {field_name} validation to fail"))?;
    if predicate(observed) {
        Ok(())
    } else {
        Err(format!("error variant mismatch for {field_name}"))
    }
}

/// Compare two values for equality, reporting a mismatch as an error.
fn ensure_eq<T, U>(actual: &T, expected: &U, context: &str) -> Result<(), String>
where
    T: PartialEq<U> + std::fmt::Debug + ?Sized,
    U: std::fmt::Debug + ?Sized,
{
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{context}: expected {expected:?}, got {actual:?}"))
    }
}

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[given("a git SHA \"{sha}\"")]
fn given_git_sha(world: &mut ArtefactWorld, sha: String) -> Result<(), String> {
    world.git_sha = Some(GitSha::try_from(sha).map_err(|e| format!("test SHA: {e}"))?);
    Ok(())
}

#[given("a toolchain channel \"{channel}\"")]
fn given_toolchain_channel(world: &mut ArtefactWorld, channel: String) -> Result<(), String> {
    world.toolchain =
        Some(ToolchainChannel::try_from(channel).map_err(|e| format!("test channel: {e}"))?);
    Ok(())
}

#[given("a target triple \"{triple}\"")]
fn given_target_triple(world: &mut ArtefactWorld, triple: String) -> Result<(), String> {
    world.target = Some(TargetTriple::try_from(triple).map_err(|e| format!("test triple: {e}"))?);
    Ok(())
}

#[when("an artefact name is constructed")]
fn when_artefact_name_constructed(world: &mut ArtefactWorld) -> Result<(), String> {
    let sha = world
        .git_sha
        .clone()
        .ok_or_else(|| String::from("git_sha set"))?;
    let ch = world
        .toolchain
        .clone()
        .ok_or_else(|| String::from("toolchain set"))?;
    let tgt = world
        .target
        .clone()
        .ok_or_else(|| String::from("target set"))?;
    world.artefact_name = Some(ArtefactName::new(sha, ch, tgt));
    Ok(())
}

#[then("the filename is \"{expected}\"")]
fn then_filename_matches(world: &mut ArtefactWorld, expected: String) -> Result<(), String> {
    let name = world
        .artefact_name
        .as_ref()
        .ok_or_else(|| String::from("artefact_name set"))?;
    ensure_eq(&name.filename(), &expected, "artefact filename")
}

#[given("an invalid target triple \"{triple}\"")]
fn given_invalid_target(world: &mut ArtefactWorld, triple: String) {
    world.target_error = TargetTriple::try_from(triple).err();
}

#[then("the target triple is rejected")]
fn then_target_rejected(world: &mut ArtefactWorld) -> Result<(), String> {
    ensure_error_matches(world.target_error.as_ref(), "target", |e| {
        matches!(e, ArtefactError::UnsupportedTarget { .. })
    })
}

#[given("all supported target triples")]
fn given_all_supported(world: &mut ArtefactWorld) {
    let all_ok = TargetTriple::supported()
        .iter()
        .all(|t| TargetTriple::try_from(*t).is_ok());
    world.all_triples_ok = Some(all_ok);
}

#[then("every triple is accepted")]
fn then_all_accepted(world: &mut ArtefactWorld) -> Result<(), String> {
    ensure_eq(&world.all_triples_ok, &Some(true), "all triples accepted")
}

#[given("an invalid git SHA \"{sha}\"")]
fn given_invalid_sha(world: &mut ArtefactWorld, sha: String) {
    world.sha_error = GitSha::try_from(sha).err();
}

#[then("the git SHA is rejected")]
fn then_sha_rejected(world: &mut ArtefactWorld) -> Result<(), String> {
    ensure_error_matches(world.sha_error.as_ref(), "SHA", |e| {
        matches!(e, ArtefactError::InvalidGitSha { .. })
    })
}

#[given("an empty toolchain channel")]
fn given_empty_channel(world: &mut ArtefactWorld) {
    world.channel_error = ToolchainChannel::try_from("").err();
}

#[then("the toolchain channel is rejected")]
fn then_channel_rejected(world: &mut ArtefactWorld) -> Result<(), String> {
    ensure_error_matches(world.channel_error.as_ref(), "channel", |e| {
        matches!(e, ArtefactError::InvalidToolchainChannel { .. })
    })
}

#[given("a complete set of manifest fields")]
fn given_manifest_fields(world: &mut ArtefactWorld) -> Result<(), String> {
    world.git_sha = Some(GitSha::try_from("abc1234").map_err(|e| format!("valid sha: {e}"))?);
    world.toolchain = Some(
        ToolchainChannel::try_from("nightly-2026-05-28")
            .map_err(|e| format!("valid channel: {e}"))?,
    );
    world.target = Some(
        TargetTriple::try_from("x86_64-unknown-linux-gnu")
            .map_err(|e| format!("valid target: {e}"))?,
    );
    Ok(())
}

#[when("a manifest is constructed")]
fn when_manifest_constructed(world: &mut ArtefactWorld) -> Result<(), String> {
    let provenance = ManifestProvenance {
        git_sha: world
            .git_sha
            .clone()
            .ok_or_else(|| String::from("git_sha set"))?,
        schema_version: SchemaVersion::current(),
        toolchain: world
            .toolchain
            .clone()
            .ok_or_else(|| String::from("toolchain set"))?,
        target: world
            .target
            .clone()
            .ok_or_else(|| String::from("target set"))?,
    };
    let digest_hex = "a".repeat(64);
    let content = ManifestContent {
        generated_at: GeneratedAt::new("2026-05-28T00:00:00Z"),
        files: vec!["libwhitaker_lints.so".to_owned()],
        sha256: Sha256Digest::try_from(digest_hex.as_str())
            .map_err(|e| format!("valid digest: {e}"))?,
    };
    world.manifest = Some(Manifest::new(provenance, content));
    Ok(())
}

#[then("all manifest fields are accessible")]
fn then_manifest_accessible(world: &mut ArtefactWorld) -> Result<(), String> {
    let m = world
        .manifest
        .as_ref()
        .ok_or_else(|| String::from("manifest set"))?;
    ensure_eq(m.git_sha().as_str(), "abc1234", "git sha")?;
    ensure_eq(&m.schema_version().as_u32(), &1, "schema version")?;
    ensure_eq(m.toolchain().as_str(), "nightly-2026-05-28", "toolchain")?;
    ensure_eq(m.target().as_str(), "x86_64-unknown-linux-gnu", "target")?;
    ensure_eq(
        m.generated_at().as_str(),
        "2026-05-28T00:00:00Z",
        "generated at",
    )?;
    ensure_eq(&m.files().len(), &1, "file count")?;
    ensure_eq(&m.sha256().as_str().len(), &64, "sha256 length")
}

#[given("the default verification policy")]
fn given_default_policy(world: &mut ArtefactWorld) {
    world.policy = Some(VerificationPolicy::default());
}

#[then("checksum verification is required")]
fn then_checksum_required(world: &mut ArtefactWorld) -> Result<(), String> {
    let policy = world
        .policy
        .as_ref()
        .ok_or_else(|| String::from("policy set"))?;
    if policy.require_checksum() {
        Ok(())
    } else {
        Err(String::from("checksum verification must be required"))
    }
}

#[given("the default failure action")]
fn given_default_failure_action(world: &mut ArtefactWorld) {
    world.failure_action = Some(VerificationFailureAction::default());
}

#[then("the action is fallback with warning")]
fn then_action_is_fallback(world: &mut ArtefactWorld) -> Result<(), String> {
    ensure_eq(
        &world.failure_action,
        &Some(VerificationFailureAction::FallbackWithWarning),
        "failure action",
    )
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Construct artefact name from valid components"
)]
fn scenario_construct_artefact_name(world: ArtefactWorld) { let _ = world; }

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Reject unsupported target triple"
)]
fn scenario_reject_unsupported_target(world: ArtefactWorld) { let _ = world; }

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Accept all five supported target triples"
)]
fn scenario_accept_all_supported_targets(world: ArtefactWorld) { let _ = world; }

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Reject invalid git SHA"
)]
fn scenario_reject_invalid_git_sha(world: ArtefactWorld) { let _ = world; }

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Reject empty toolchain channel"
)]
fn scenario_reject_empty_channel(world: ArtefactWorld) { let _ = world; }

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Construct manifest with all fields"
)]
fn scenario_construct_manifest(world: ArtefactWorld) { let _ = world; }

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Default verification policy requires checksum"
)]
fn scenario_default_verification_policy(world: ArtefactWorld) { let _ = world; }

#[scenario(
    path = "tests/features/artefact_policy.feature",
    name = "Verification failure triggers fallback"
)]
fn scenario_verification_failure_fallback(world: ArtefactWorld) { let _ = world; }
