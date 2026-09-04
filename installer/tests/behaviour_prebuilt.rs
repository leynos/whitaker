//! BDD tests for the prebuilt artefact download and verification workflow.

use std::{
    path::Path,
    sync::{Mutex, PoisonError},
};

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use rstest::fixture;
use rstest_bdd_macros::{given, then, when};

#[path = "behaviour_prebuilt/filesystem.rs"]
mod filesystem;
#[path = "behaviour_prebuilt/scenarios.rs"]
mod scenarios;
use filesystem::write_file;
use whitaker_installer::{
    artefact::{
        download::{ArtefactDownloader, DownloadError},
        extraction::{ArtefactExtractor, ExtractionError},
    },
    cli::{Cli, InstallArgs},
    prebuilt::{PrebuiltConfig, PrebuiltResult, attempt_prebuilt_with},
    resolution::{CrateResolutionOptions, resolve_crates},
    test_utils::{prebuilt_manifest_json, sha256_hex},
};

const FAKE_ARCHIVE: &[u8] = b"fake archive content";
const DEFAULT_TARGET: &str = "x86_64-unknown-linux-gnu";
const DEFAULT_TOOLCHAIN: &str = "nightly-2026-05-28";

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
    const fn new(manifest: ManifestBehaviour, archive: ArchiveBehaviour) -> Self {
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
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or_else(|| {
                DownloadError::Io(std::io::Error::other(
                    "stub manifest behaviour was not configured",
                ))
            })?;
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
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .unwrap_or_default();
        match behaviour {
            ArchiveBehaviour::CorrectChecksum => {
                let destination = Utf8Path::from_path(dest).ok_or_else(|| {
                    DownloadError::Io(std::io::Error::other(
                        "archive destination path must be valid UTF-8",
                    ))
                })?;
                write_file(destination, FAKE_ARCHIVE).map_err(DownloadError::Io)
            }
            ArchiveBehaviour::WrongChecksum => {
                let destination = Utf8Path::from_path(dest).ok_or_else(|| {
                    DownloadError::Io(std::io::Error::other(
                        "archive destination path must be valid UTF-8",
                    ))
                })?;
                write_file(destination, b"tampered content").map_err(DownloadError::Io)
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
        dest_dir: &Path,
    ) -> Result<Vec<String>, ExtractionError> {
        let source_name = "libwhitaker_suite.so".to_owned();
        let destination_dir = Utf8Path::from_path(dest_dir).ok_or_else(|| {
            ExtractionError::Io(std::io::Error::other(
                "extraction destination path must be valid UTF-8",
            ))
        })?;
        write_file(&destination_dir.join(&source_name), b"fake").map_err(ExtractionError::Io)?;
        Ok(vec![source_name])
    }
}

#[derive(Default)]
struct PrebuiltWorld {
    _temp_dir: Option<tempfile::TempDir>,
    staging_root: Option<Utf8PathBuf>,
    expected_toolchain: Option<String>,
    requested_target: Option<String>,
    manifest_behaviour: Option<ManifestBehaviour>,
    archive_behaviour: Option<ArchiveBehaviour>,
    result: Option<PrebuiltResult>,
    install_args: Option<InstallArgs>,
    should_attempt_prebuilt: Option<bool>,
    force_destination_conflict: bool,
    attempted_destination: Option<Utf8PathBuf>,
}

// A fixture cannot report failure to the scenario binding, so setup problems
// abort the scenario with a descriptive panic.
#[fixture]
fn world() -> PrebuiltWorld {
    let Ok(temp_dir) = tempfile::tempdir() else {
        panic!("failed to create staging temp dir");
    };
    let Ok(staging_root) = Utf8PathBuf::try_from(temp_dir.path().to_path_buf()) else {
        panic!("staging temp dir path is not valid UTF-8");
    };
    PrebuiltWorld {
        _temp_dir: Some(temp_dir),
        staging_root: Some(staging_root),
        expected_toolchain: Some(DEFAULT_TOOLCHAIN.to_owned()),
        requested_target: Some(DEFAULT_TARGET.to_owned()),
        ..Default::default()
    }
}

#[given("a valid manifest for target \"{target}\"")]
fn given_valid_manifest(world: &mut PrebuiltWorld, target: String) {
    let sha = sha256_hex(FAKE_ARCHIVE);
    world.requested_target = Some(target.clone());
    world.manifest_behaviour = Some(ManifestBehaviour::Ok(prebuilt_manifest_json(
        DEFAULT_TOOLCHAIN,
        &target,
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
    let target = world
        .requested_target
        .clone()
        .unwrap_or_else(|| DEFAULT_TARGET.to_owned());
    world.manifest_behaviour = Some(ManifestBehaviour::Ok(prebuilt_manifest_json(
        DEFAULT_TOOLCHAIN,
        &target,
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
    let target = world
        .requested_target
        .clone()
        .unwrap_or_else(|| DEFAULT_TARGET.to_owned());
    let sha = sha256_hex(FAKE_ARCHIVE);
    world.manifest_behaviour = Some(ManifestBehaviour::Ok(prebuilt_manifest_json(
        &toolchain, &target, &sha,
    )));
}

#[given("the expected toolchain is \"{toolchain}\"")]
fn given_expected_toolchain(world: &mut PrebuiltWorld, toolchain: String) {
    world.expected_toolchain = Some(toolchain);
}

#[given("the build-only flag is set")]
fn given_build_only(world: &mut PrebuiltWorld) {
    let cli = Cli::parse_from(["whitaker-installer", "--build-only"]);
    world.install_args = Some(cli.install.clone());
}

#[given("the destination path cannot be created")]
fn given_destination_path_conflict(world: &mut PrebuiltWorld) {
    world.force_destination_conflict = true;
}

#[when("prebuilt download is attempted")]
fn when_prebuilt_attempted(world: &mut PrebuiltWorld) -> Result<(), String> {
    let toolchain = world
        .expected_toolchain
        .as_deref()
        .unwrap_or(DEFAULT_TOOLCHAIN);
    let target = world.requested_target.as_deref().unwrap_or(DEFAULT_TARGET);
    let staging_root = world
        .staging_root
        .as_ref()
        .ok_or_else(|| String::from("staging_root set"))?;
    let destination_dir = if world.force_destination_conflict {
        let occupied = staging_root.join("occupied");
        write_file(&occupied, b"occupied file")
            .map_err(|error| format!("write occupied file: {error}"))?;
        occupied.join("child").join("lib")
    } else {
        staging_root
            .join("lints")
            .join(toolchain)
            .join(target)
            .join("lib")
    };
    world.attempted_destination = Some(destination_dir.clone());
    let config = PrebuiltConfig {
        target,
        toolchain,
        destination_dir: &destination_dir,
        quiet: true,
    };

    let manifest_behaviour = world
        .manifest_behaviour
        .take()
        .ok_or_else(|| String::from("manifest_behaviour set"))?;
    let archive_behaviour = world
        .archive_behaviour
        .take()
        .unwrap_or(ArchiveBehaviour::CorrectChecksum);

    let downloader = StubDownloader::new(manifest_behaviour, archive_behaviour);
    let extractor = StubExtractor;

    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    world.result = Some(result);
    Ok(())
}

#[when("the install configuration is checked")]
fn when_install_config_checked(world: &mut PrebuiltWorld) {
    let install_args = world.install_args.clone().unwrap_or_default();
    let options = CrateResolutionOptions {
        individual_lints: install_args.individual_lints,
        experimental: install_args.experimental,
    };
    let requested_crates = resolve_crates(&[], &options);
    world.should_attempt_prebuilt = Some(install_args.should_attempt_prebuilt(&requested_crates));
}

/// Borrow the recorded prebuilt outcome, failing when the When step has not run.
fn prebuilt_result(world: &PrebuiltWorld) -> Result<&PrebuiltResult, String> {
    world
        .result
        .as_ref()
        .ok_or_else(|| String::from("result set"))
}

#[then("the prebuilt result is success")]
fn then_result_is_success(world: &mut PrebuiltWorld) -> Result<(), String> {
    let result = prebuilt_result(world)?;
    if matches!(result, PrebuiltResult::Success { .. }) {
        Ok(())
    } else {
        Err(format!("expected Success, got {result:?}"))
    }
}

#[then("the staging path uses toolchain, target, and lib directories")]
fn then_staging_path_uses_expected_layout(world: &mut PrebuiltWorld) -> Result<(), String> {
    let result = prebuilt_result(world)?;
    let PrebuiltResult::Success { staging_path } = result else {
        return Err(format!("expected Success, got {result:?}"));
    };
    let toolchain = world
        .expected_toolchain
        .as_deref()
        .unwrap_or(DEFAULT_TOOLCHAIN);
    let target = world.requested_target.as_deref().unwrap_or(DEFAULT_TARGET);
    let expected_suffix = format!("{toolchain}/{target}/lib");
    if staging_path.ends_with(&expected_suffix) {
        Ok(())
    } else {
        Err(format!(
            "staging path {staging_path} does not end with {expected_suffix}"
        ))
    }
}

#[then("the prebuilt result is fallback")]
fn then_result_is_fallback(world: &mut PrebuiltWorld) -> Result<(), String> {
    let result = prebuilt_result(world)?;
    if matches!(result, PrebuiltResult::Fallback { .. }) {
        Ok(())
    } else {
        Err(format!("expected Fallback, got {result:?}"))
    }
}

#[then("the fallback reason mentions \"{keyword}\"")]
fn then_fallback_reason_mentions(world: &mut PrebuiltWorld, keyword: String) -> Result<(), String> {
    let result = prebuilt_result(world)?;
    let PrebuiltResult::Fallback { reason } = result else {
        return Err(format!("expected Fallback, got {result:?}"));
    };
    let lower_reason = reason.to_lowercase();
    let lower_keyword = keyword.to_lowercase();
    if lower_reason.contains(&lower_keyword) {
        Ok(())
    } else {
        Err(format!(
            "expected reason to contain '{keyword}', got: {reason}"
        ))
    }
}

#[then("no prebuilt download is attempted")]
fn then_no_prebuilt_attempted(world: &mut PrebuiltWorld) -> Result<(), String> {
    if world.should_attempt_prebuilt == Some(false) {
        Ok(())
    } else {
        Err(String::from(
            "expected no prebuilt download attempt when --build-only is set",
        ))
    }
}

#[then("the destination directory is not created")]
fn then_destination_is_not_created(world: &mut PrebuiltWorld) -> Result<(), String> {
    let destination = world
        .attempted_destination
        .as_ref()
        .ok_or_else(|| String::from("attempted destination should be set"))?;
    if destination.exists() {
        Err(format!(
            "destination directory should not exist: {destination}"
        ))
    } else {
        Ok(())
    }
}
