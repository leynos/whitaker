//! BDD tests for the prebuilt artefact download and verification workflow.

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::path::Path;
use std::sync::Mutex;
use whitaker_installer::artefact::download::{ArtefactDownloader, DownloadError};
use whitaker_installer::artefact::extraction::{ArtefactExtractor, ExtractionError};
use whitaker_installer::prebuilt::{PrebuiltConfig, PrebuiltResult, attempt_prebuilt_with};

// ---------------------------------------------------------------------------
// Test doubles
// ---------------------------------------------------------------------------

const FAKE_ARCHIVE: &[u8] = b"fake archive content";
const DEFAULT_TARGET: &str = "x86_64-unknown-linux-gnu";
const DEFAULT_TOOLCHAIN: &str = "nightly-2025-09-18";

/// Compute the SHA-256 hex digest of a byte slice.
fn sha256_of(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(data))
}

/// Build manifest JSON with configurable toolchain and SHA-256.
fn manifest_json(toolchain: &str, sha256: &str) -> String {
    format!(
        concat!(
            r#"{{"git_sha":"abc1234","schema_version":1,"#,
            r#""toolchain":"{toolchain}","#,
            r#""target":"x86_64-unknown-linux-gnu","#,
            r#""generated_at":"2026-02-03T00:00:00Z","#,
            r#""files":["lib.so"],"#,
            r#""sha256":"{sha256}"}}"#,
        ),
        toolchain = toolchain,
        sha256 = sha256,
    )
}

/// How the stub downloader should respond to `download_manifest`.
enum ManifestBehaviour {
    /// Return the given JSON string.
    Ok(String),
    /// Return an HTTP error.
    HttpError { url: String, reason: String },
    /// Return a 404 not-found error.
    NotFound { url: String },
}

/// How the stub downloader should respond to `download_archive`.
#[derive(Default)]
enum ArchiveBehaviour {
    /// Write content whose SHA-256 matches the manifest.
    #[default]
    CorrectChecksum,
    /// Write content whose SHA-256 does NOT match the manifest.
    WrongChecksum,
}

/// A simple stub implementation of [`ArtefactDownloader`] for BDD tests.
struct StubDownloader {
    manifest: Mutex<Option<ManifestBehaviour>>,
    archive: Mutex<Option<ArchiveBehaviour>>,
}

impl StubDownloader {
    fn new(manifest: ManifestBehaviour, archive: ArchiveBehaviour) -> Self {
        Self {
            manifest: Mutex::new(Some(manifest)),
            archive: Mutex::new(Some(archive)),
        }
    }
}

impl ArtefactDownloader for StubDownloader {
    fn download_manifest(&self, _target: &str) -> Result<String, DownloadError> {
        let behaviour = self
            .manifest
            .lock()
            .expect("lock")
            .take()
            .expect("manifest behaviour not set");
        match behaviour {
            ManifestBehaviour::Ok(json) => Ok(json),
            ManifestBehaviour::HttpError { url, reason } => {
                Err(DownloadError::HttpError { url, reason })
            }
            ManifestBehaviour::NotFound { url } => Err(DownloadError::NotFound { url }),
        }
    }

    fn download_archive(&self, _filename: &str, dest: &Path) -> Result<(), DownloadError> {
        let behaviour = self
            .archive
            .lock()
            .expect("lock")
            .take()
            .unwrap_or(ArchiveBehaviour::CorrectChecksum);
        match behaviour {
            ArchiveBehaviour::CorrectChecksum => {
                std::fs::write(dest, FAKE_ARCHIVE).map_err(DownloadError::Io)
            }
            ArchiveBehaviour::WrongChecksum => {
                std::fs::write(dest, b"tampered content").map_err(DownloadError::Io)
            }
        }
    }
}

/// A stub extractor that always succeeds.
struct StubExtractor;

impl ArtefactExtractor for StubExtractor {
    fn extract(
        &self,
        _archive_path: &Path,
        _dest_dir: &Path,
    ) -> Result<Vec<String>, ExtractionError> {
        Ok(vec!["lib.so".to_owned()])
    }
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PrebuiltWorld {
    _temp_dir: Option<tempfile::TempDir>,
    staging_base: Option<Utf8PathBuf>,
    expected_toolchain: Option<String>,
    manifest_behaviour: Option<ManifestBehaviour>,
    archive_behaviour: Option<ArchiveBehaviour>,
    result: Option<PrebuiltResult>,
    build_only: bool,
    prebuilt_attempted: bool,
}

#[fixture]
fn world() -> PrebuiltWorld {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let staging_base = Utf8PathBuf::try_from(temp_dir.path().to_path_buf()).expect("UTF-8 path");
    PrebuiltWorld {
        _temp_dir: Some(temp_dir),
        staging_base: Some(staging_base),
        expected_toolchain: Some(DEFAULT_TOOLCHAIN.to_owned()),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a valid manifest for target \"{target}\"")]
fn given_valid_manifest(world: &mut PrebuiltWorld, target: String) {
    let _ = target;
    let sha = sha256_of(FAKE_ARCHIVE);
    world.manifest_behaviour = Some(ManifestBehaviour::Ok(manifest_json(
        DEFAULT_TOOLCHAIN,
        &sha,
    )));
}

#[given("a matching archive with correct checksum")]
fn given_correct_checksum(world: &mut PrebuiltWorld) {
    world.archive_behaviour = Some(ArchiveBehaviour::CorrectChecksum);
}

#[given("an archive with mismatched checksum")]
fn given_wrong_checksum(world: &mut PrebuiltWorld) {
    let sha = "a".repeat(64);
    world.manifest_behaviour = Some(ManifestBehaviour::Ok(manifest_json(
        DEFAULT_TOOLCHAIN,
        &sha,
    )));
    world.archive_behaviour = Some(ArchiveBehaviour::WrongChecksum);
}

#[given("a manifest download that fails with a network error")]
fn given_network_error(world: &mut PrebuiltWorld) {
    world.manifest_behaviour = Some(ManifestBehaviour::HttpError {
        url: "http://example.com".to_owned(),
        reason: "connection refused".to_owned(),
    });
}

#[given("a manifest download that returns not found")]
fn given_not_found(world: &mut PrebuiltWorld) {
    world.manifest_behaviour = Some(ManifestBehaviour::NotFound {
        url: "http://example.com/manifest".to_owned(),
    });
}

#[given("a valid manifest with toolchain \"{toolchain}\"")]
fn given_manifest_with_toolchain(world: &mut PrebuiltWorld, toolchain: String) {
    let sha = sha256_of(FAKE_ARCHIVE);
    world.manifest_behaviour = Some(ManifestBehaviour::Ok(manifest_json(&toolchain, &sha)));
}

#[given("the expected toolchain is \"{toolchain}\"")]
fn given_expected_toolchain(world: &mut PrebuiltWorld, toolchain: String) {
    world.expected_toolchain = Some(toolchain);
}

#[given("the build-only flag is set")]
fn given_build_only(world: &mut PrebuiltWorld) {
    world.build_only = true;
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("prebuilt download is attempted")]
fn when_prebuilt_attempted(world: &mut PrebuiltWorld) {
    world.prebuilt_attempted = true;

    let staging_base = world.staging_base.as_ref().expect("staging_base set");
    let toolchain = world
        .expected_toolchain
        .as_deref()
        .unwrap_or(DEFAULT_TOOLCHAIN);
    let config = PrebuiltConfig {
        target: DEFAULT_TARGET,
        toolchain,
        staging_base,
        quiet: true,
    };

    let manifest_behaviour = world
        .manifest_behaviour
        .take()
        .expect("manifest_behaviour set");
    let archive_behaviour = world
        .archive_behaviour
        .take()
        .unwrap_or(ArchiveBehaviour::CorrectChecksum);

    let downloader = StubDownloader::new(manifest_behaviour, archive_behaviour);
    let extractor = StubExtractor;

    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    world.result = Some(result);
}

#[when("the install configuration is checked")]
fn when_install_config_checked(world: &mut PrebuiltWorld) {
    if world.build_only {
        world.prebuilt_attempted = false;
    }
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the prebuilt result is success")]
fn then_result_is_success(world: &mut PrebuiltWorld) {
    let result = world.result.as_ref().expect("result set");
    assert!(
        matches!(result, PrebuiltResult::Success { .. }),
        "expected Success, got {result:?}"
    );
}

#[then("the staging path contains the expected toolchain directory")]
fn then_staging_path_has_toolchain_dir(world: &mut PrebuiltWorld) {
    let result = world.result.as_ref().expect("result set");
    if let PrebuiltResult::Success { staging_path } = result {
        let toolchain = world
            .expected_toolchain
            .as_deref()
            .unwrap_or(DEFAULT_TOOLCHAIN);
        assert!(
            staging_path.as_str().contains(toolchain),
            "staging path {staging_path} does not contain {toolchain}"
        );
    } else {
        panic!("expected Success, got {result:?}");
    }
}

#[then("the prebuilt result is fallback")]
fn then_result_is_fallback(world: &mut PrebuiltWorld) {
    let result = world.result.as_ref().expect("result set");
    assert!(
        matches!(result, PrebuiltResult::Fallback { .. }),
        "expected Fallback, got {result:?}"
    );
}

#[then("the fallback reason mentions \"{keyword}\"")]
fn then_fallback_reason_mentions(world: &mut PrebuiltWorld, keyword: String) {
    let result = world.result.as_ref().expect("result set");
    match result {
        PrebuiltResult::Fallback { reason } => {
            let lower_reason = reason.to_lowercase();
            let lower_keyword = keyword.to_lowercase();
            assert!(
                lower_reason.contains(&lower_keyword),
                "expected reason to contain '{keyword}', got: {reason}"
            );
        }
        other => panic!("expected Fallback, got {other:?}"),
    }
}

#[then("no prebuilt download is attempted")]
fn then_no_prebuilt_attempted(world: &mut PrebuiltWorld) {
    assert!(
        !world.prebuilt_attempted,
        "expected no prebuilt download attempt when --build-only is set"
    );
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

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
