//! Unit tests for prebuilt artefact orchestration.

use super::*;
use crate::artefact::download::MockArtefactDownloader;
use crate::artefact::extraction::MockArtefactExtractor;
use crate::test_utils::{prebuilt_manifest_json, sha256_hex};
use rstest::rstest;

const FAKE_ARCHIVE: &[u8] = b"fake archive content";
const TARGET: &str = "x86_64-unknown-linux-gnu";
const TOOLCHAIN: &str = "nightly-2025-09-18";

fn base_config(staging_base: &Utf8Path) -> PrebuiltConfig<'_> {
    PrebuiltConfig {
        target: TARGET,
        toolchain: TOOLCHAIN,
        staging_base,
        quiet: true,
    }
}

fn staging_base() -> (tempfile::TempDir, Utf8PathBuf) {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = Utf8PathBuf::try_from(temp.path().to_path_buf()).expect("UTF-8 path");
    (temp, path)
}

/// Run a fallback scenario: set up mocks via `setup_mocks`, call the
/// orchestrator, and assert `Fallback` whose reason contains
/// `expected_reason_substring`.
fn test_fallback_scenario(
    setup_mocks: impl FnOnce(&mut MockArtefactDownloader, &mut MockArtefactExtractor),
    expected_reason_substring: &str,
) {
    let (_temp, staging_base) = staging_base();
    let config = base_config(&staging_base);

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
    let (_temp, staging_base) = staging_base();
    let config = base_config(&staging_base);
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
    assert!(
        matches!(result, PrebuiltResult::Success { .. }),
        "expected Success, got {result:?}"
    );
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
fn toolchain_mismatch_returns_fallback() {
    test_fallback_scenario(
        |downloader, _extractor| {
            let manifest_json =
                prebuilt_manifest_json("nightly-2025-01-01", TARGET, &"a".repeat(64));
            downloader
                .expect_download_manifest()
                .returning(move |_| Ok(manifest_json.clone()));
        },
        "toolchain mismatch",
    );
}

#[test]
fn checksum_mismatch_returns_fallback() {
    test_fallback_scenario(
        |downloader, _extractor| {
            // Manifest claims SHA = "aaa...a" but the file will hash differently.
            let manifest_json = prebuilt_manifest_json(TOOLCHAIN, TARGET, &"a".repeat(64));
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
fn target_mismatch_returns_fallback() {
    test_fallback_scenario(
        |downloader, _extractor| {
            let manifest_json =
                prebuilt_manifest_json(TOOLCHAIN, "aarch64-apple-darwin", &"a".repeat(64));
            downloader
                .expect_download_manifest()
                .returning(move |_| Ok(manifest_json.clone()));
        },
        "target mismatch",
    );
}
