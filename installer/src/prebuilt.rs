//! Prebuilt artefact download and verification orchestrator.
//!
//! Implements the download-first strategy from ADR-001: before compiling
//! locally, the installer attempts to download a prebuilt `.tar.zst`
//! archive from the GitHub rolling release, verify its SHA-256 checksum
//! against the manifest, and extract the libraries to the staging
//! directory.  On any failure the caller receives [`PrebuiltResult::Fallback`]
//! and should proceed with local compilation.

use camino::{Utf8Path, Utf8PathBuf};
use std::io::Write;
use std::path::Path;

use crate::artefact::download::{ArtefactDownloader, DownloadError, HttpDownloader};
use crate::artefact::extraction::{ArtefactExtractor, ZstdExtractor};
use crate::artefact::manifest::Manifest;
use crate::artefact::manifest_parser::{ManifestParseError, parse_manifest};
use crate::artefact::naming::ArtefactName;
use crate::artefact::packaging::compute_sha256;
use crate::artefact::packaging_error::PackagingError;
use crate::artefact::verification::VerificationPolicy;
use crate::output::write_stderr_line;

/// The outcome of a prebuilt download attempt.
///
/// This is deliberately not a `Result` — prebuilt failures are never
/// fatal.  Callers pattern-match and fall back to local compilation
/// on [`Fallback`](PrebuiltResult::Fallback).
#[derive(Debug)]
pub enum PrebuiltResult {
    /// The prebuilt archive was downloaded, verified, and extracted.
    Success {
        /// Path to the directory containing the extracted libraries.
        staging_path: Utf8PathBuf,
    },
    /// The prebuilt attempt failed; the caller should build locally.
    Fallback {
        /// A human-readable explanation of why the fallback occurred.
        reason: String,
    },
}

/// Configuration for a prebuilt download attempt.
#[derive(Debug)]
pub struct PrebuiltConfig<'a> {
    /// The host target triple (e.g. `x86_64-unknown-linux-gnu`).
    pub target: &'a str,
    /// The expected toolchain channel (e.g. `nightly-2025-09-18`).
    pub toolchain: &'a str,
    /// The base staging directory for extracted libraries.
    pub staging_base: &'a Utf8Path,
    /// When true, suppress progress output.
    pub quiet: bool,
}

/// Internal error type for the prebuilt pipeline.
///
/// Not exported — all variants are mapped to
/// [`PrebuiltResult::Fallback`] with a descriptive reason string.
#[derive(Debug, thiserror::Error)]
enum PrebuiltError {
    #[error("download failed: {0}")]
    Download(#[from] DownloadError),

    #[error("manifest parse failed: {0}")]
    ManifestParse(#[from] ManifestParseError),

    #[error("toolchain mismatch: manifest has {manifest}, expected {expected}")]
    ToolchainMismatch { manifest: String, expected: String },

    #[error("checksum mismatch: manifest={expected}, actual={actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("checksum computation failed: {0}")]
    ChecksumCompute(PackagingError),

    #[error("extraction failed: {0}")]
    Extraction(#[from] crate::artefact::extraction::ExtractionError),
}

/// Attempt to download and install prebuilt artefacts using production
/// HTTP and extraction implementations.
///
/// Returns [`PrebuiltResult::Success`] with the staging path on success,
/// or [`PrebuiltResult::Fallback`] with a reason on any failure.
pub fn attempt_prebuilt(config: &PrebuiltConfig<'_>, stderr: &mut dyn Write) -> PrebuiltResult {
    attempt_prebuilt_with(config, &HttpDownloader, &ZstdExtractor, stderr)
}

/// Testable inner function with injected dependencies.
///
/// The production entry point [`attempt_prebuilt`] delegates here with
/// real implementations; tests inject mocks.
///
/// This function is public to allow integration tests to inject mock
/// downloader and extractor implementations.
pub fn attempt_prebuilt_with(
    config: &PrebuiltConfig<'_>,
    downloader: &dyn ArtefactDownloader,
    extractor: &dyn ArtefactExtractor,
    stderr: &mut dyn Write,
) -> PrebuiltResult {
    match run_pipeline(config, downloader, extractor, stderr) {
        Ok(staging_path) => PrebuiltResult::Success { staging_path },
        Err(e) => {
            let reason = e.to_string();
            if !config.quiet {
                write_stderr_line(stderr, format!("Prebuilt download unavailable: {reason}"));
                write_stderr_line(stderr, "Falling back to local compilation.");
                write_stderr_line(stderr, "");
            }
            PrebuiltResult::Fallback { reason }
        }
    }
}

/// The core pipeline: download → parse → verify → extract.
fn run_pipeline(
    config: &PrebuiltConfig<'_>,
    downloader: &dyn ArtefactDownloader,
    extractor: &dyn ArtefactExtractor,
    stderr: &mut dyn Write,
) -> Result<Utf8PathBuf, PrebuiltError> {
    // Step 1: Download manifest.
    if !config.quiet {
        write_stderr_line(
            stderr,
            format!("Checking for prebuilt artefacts for {}...", config.target),
        );
    }
    let manifest_json = downloader.download_manifest(config.target)?;

    // Step 2: Parse and validate manifest.
    let manifest = parse_manifest(&manifest_json)?;
    validate_toolchain(&manifest, config.toolchain)?;

    // Step 3: Derive archive filename and download.
    let archive_filename = derive_archive_filename(&manifest);
    let temp_dir =
        tempfile::tempdir().map_err(|e| PrebuiltError::Download(DownloadError::Io(e)))?;
    let archive_path = temp_dir.path().join(&archive_filename);

    if !config.quiet {
        write_stderr_line(stderr, format!("Downloading {archive_filename}..."));
    }
    downloader.download_archive(&archive_filename, &archive_path)?;

    // Step 4: Verify checksum if required by policy.
    let policy = VerificationPolicy::default();
    if policy.require_checksum() {
        verify_checksum(&manifest, &archive_path)?;
    }

    // Step 5: Extract to staging directory.
    let staging_path = config.staging_base.join(config.toolchain).join("release");
    std::fs::create_dir_all(staging_path.as_std_path())
        .map_err(|e| PrebuiltError::Download(DownloadError::Io(e)))?;

    if !config.quiet {
        write_stderr_line(stderr, "Extracting prebuilt libraries...");
    }
    extractor.extract(&archive_path, staging_path.as_std_path())?;

    if !config.quiet {
        write_stderr_line(stderr, "Prebuilt libraries installed successfully.");
        write_stderr_line(stderr, "");
    }

    Ok(staging_path)
}

/// Validate that the manifest toolchain matches the expected channel.
fn validate_toolchain(manifest: &Manifest, expected: &str) -> Result<(), PrebuiltError> {
    if manifest.toolchain().as_str() != expected {
        return Err(PrebuiltError::ToolchainMismatch {
            manifest: manifest.toolchain().to_string(),
            expected: expected.to_owned(),
        });
    }
    Ok(())
}

/// Derive the expected archive filename from manifest fields.
fn derive_archive_filename(manifest: &Manifest) -> String {
    let name = ArtefactName::new(
        manifest.git_sha().clone(),
        manifest.toolchain().clone(),
        manifest.target().clone(),
    );
    name.filename()
}

/// Verify the downloaded archive checksum against the manifest digest.
fn verify_checksum(manifest: &Manifest, archive_path: &Path) -> Result<(), PrebuiltError> {
    let actual = compute_sha256(archive_path).map_err(PrebuiltError::ChecksumCompute)?;
    if actual.as_str() != manifest.sha256().as_str() {
        return Err(PrebuiltError::ChecksumMismatch {
            expected: manifest.sha256().to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "prebuilt_tests.rs"]
mod tests;
