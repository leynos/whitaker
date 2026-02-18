//! Unit tests for prebuilt artefact orchestration.

use super::*;
use crate::artefact::download::MockArtefactDownloader;
use crate::artefact::extraction::MockArtefactExtractor;
use crate::test_utils::{prebuilt_manifest_json, sha256_hex};
use rstest::rstest;

const FAKE_ARCHIVE: &[u8] = b"fake archive content";
const TARGET: &str = "x86_64-unknown-linux-gnu";
const TOOLCHAIN: &str = "nightly-2025-09-18";

fn base_config(destination_dir: &Utf8Path) -> PrebuiltConfig<'_> {
    PrebuiltConfig {
        target: TARGET,
        toolchain: TOOLCHAIN,
        destination_dir,
        quiet: true,
    }
}

fn destination_dir() -> (tempfile::TempDir, Utf8PathBuf) {
    let temp = tempfile::tempdir().expect("temp dir");
    let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).expect("UTF-8 path");
    let path = root.join("lints").join(TOOLCHAIN).join(TARGET).join("lib");
    (temp, path)
}

/// Run a fallback scenario: set up mocks via `setup_mocks`, call the
/// orchestrator, and assert `Fallback` whose reason contains
/// `expected_reason_substring`.
fn test_fallback_scenario(
    setup_mocks: impl FnOnce(&mut MockArtefactDownloader, &mut MockArtefactExtractor),
    expected_reason_substring: &str,
) {
    let (_temp, destination_dir) = destination_dir();
    let config = base_config(&destination_dir);

    let mut downloader = MockArtefactDownloader::new();
    let mut extractor = MockArtefactExtractor::new();
    setup_mocks(&mut downloader, &mut extractor);

    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    match result {
        PrebuiltResult::Fallback { reason } => {
            assert!(
                reason.contains(expected_reason_substring),
                "reason: {reason}"
            );
        }
        other => panic!("expected Fallback, got {other:?}"),
    }
}

#[test]
fn happy_path_returns_success() {
    let (_temp, destination_dir) = destination_dir();
    let config = base_config(&destination_dir);
    let fake_sha = sha256_hex(FAKE_ARCHIVE);
    let manifest_json = prebuilt_manifest_json(TOOLCHAIN, TARGET, &fake_sha);

    let mut downloader = MockArtefactDownloader::new();
    downloader
        .expect_download_manifest()
        .returning(move |_| Ok(manifest_json.clone()));
    downloader
        .expect_download_archive()
        .returning(|_filename, dest| std::fs::write(dest, FAKE_ARCHIVE).map_err(DownloadError::Io));

    let mut extractor = MockArtefactExtractor::new();
    extractor.expect_extract().returning(|_archive, dest| {
        let source_name = "libwhitaker_suite.so".to_owned();
        std::fs::write(dest.join(&source_name), b"fake").expect("write extracted file");
        Ok(vec![source_name])
    });

    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    match result {
        PrebuiltResult::Success { staging_path } => assert_eq!(staging_path, destination_dir),
        other => panic!("expected Success, got {other:?}"),
    }
}

#[rstest]
#[case::http_error(make_http_error, "download")]
#[case::not_found(make_not_found_error, "not found")]
fn manifest_download_errors_return_fallback(
    #[case] make_error: fn() -> DownloadError,
    #[case] expected_substring: &str,
) {
    test_fallback_scenario(
        |downloader, _extractor| {
            downloader
                .expect_download_manifest()
                .returning(move |_| Err(make_error()));
        },
        expected_substring,
    );
}

fn make_http_error() -> DownloadError {
    DownloadError::HttpError {
        url: "http://example.com".to_owned(),
        reason: "connection refused".to_owned(),
    }
}

fn make_not_found_error() -> DownloadError {
    DownloadError::NotFound {
        url: "http://example.com/manifest".to_owned(),
    }
}

#[test]
fn manifest_validation_errors_return_fallback() {
    let test_cases = vec![
        (
            "toolchain mismatch",
            "nightly-2025-01-01",
            TARGET,
            "toolchain mismatch",
        ),
        (
            "target mismatch",
            TOOLCHAIN,
            "aarch64-apple-darwin",
            "target mismatch",
        ),
    ];

    for (case_name, toolchain, target, expected_reason_substring) in test_cases {
        test_fallback_scenario(
            |downloader, _extractor| {
                let manifest_json = prebuilt_manifest_json(toolchain, target, "a".repeat(64));
                downloader
                    .expect_download_manifest()
                    .returning(move |_| Ok(manifest_json.clone()));
            },
            expected_reason_substring,
        );
        eprintln!("manifest validation scenario passed: {case_name}");
    }
}

#[test]
fn checksum_mismatch_returns_fallback() {
    test_fallback_scenario(
        |downloader, _extractor| {
            // Manifest claims SHA = "aaa...a" but the file will hash differently.
            let manifest_json = prebuilt_manifest_json(TOOLCHAIN, TARGET, "a".repeat(64));
            downloader
                .expect_download_manifest()
                .returning(move |_| Ok(manifest_json.clone()));
            downloader
                .expect_download_archive()
                .returning(|_filename, dest| {
                    std::fs::write(dest, b"wrong content").map_err(DownloadError::Io)
                });
        },
        "checksum mismatch",
    );
}

#[test]
fn extraction_failure_returns_fallback() {
    test_fallback_scenario(
        |downloader, extractor| {
            let fake_sha = sha256_hex(FAKE_ARCHIVE);
            let manifest_json = prebuilt_manifest_json(TOOLCHAIN, TARGET, &fake_sha);
            downloader
                .expect_download_manifest()
                .returning(move |_| Ok(manifest_json.clone()));
            downloader
                .expect_download_archive()
                .returning(|_filename, dest| {
                    std::fs::write(dest, FAKE_ARCHIVE).map_err(DownloadError::Io)
                });
            extractor.expect_extract().returning(|_archive, _dest| {
                Err(crate::artefact::extraction::ExtractionError::EmptyArchive)
            });
        },
        "extraction",
    );
}

#[test]
fn destination_creation_failure_returns_fallback() {
    let temp = tempfile::tempdir().expect("temp dir");
    let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).expect("UTF-8 path");
    let occupied = root.join("occupied");
    std::fs::write(occupied.as_std_path(), b"file").expect("write occupied file");
    let destination_dir = occupied.join("child").join("lib");
    let config = base_config(&destination_dir);

    let fake_sha = sha256_hex(FAKE_ARCHIVE);
    let manifest_json = prebuilt_manifest_json(TOOLCHAIN, TARGET, &fake_sha);

    let mut downloader = MockArtefactDownloader::new();
    downloader
        .expect_download_manifest()
        .returning(move |_| Ok(manifest_json.clone()));
    downloader
        .expect_download_archive()
        .returning(|_filename, dest| std::fs::write(dest, FAKE_ARCHIVE).map_err(DownloadError::Io));

    let extractor = MockArtefactExtractor::new();
    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    match result {
        PrebuiltResult::Fallback { reason } => assert!(
            reason.contains("download failed"),
            "unexpected fallback reason: {reason}"
        ),
        other => panic!("expected Fallback, got {other:?}"),
    }
}
