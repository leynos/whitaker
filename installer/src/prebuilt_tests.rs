//! Unit tests for prebuilt artefact orchestration.

use super::*;
use crate::artefact::download::MockArtefactDownloader;
use crate::artefact::extraction::MockArtefactExtractor;
use crate::git::CommitSha;
use crate::test_utils::{prebuilt_manifest_json, sha256_hex};
use cap_std::{ambient_authority, fs::Dir};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use rstest::rstest;
use std::path::Path;

const FAKE_ARCHIVE: &[u8] = b"fake archive content";
const TARGET: &str = "x86_64-unknown-linux-gnu";
const TOOLCHAIN: &str = "nightly-2026-05-28";

/// A full 40-hex commit SHA beginning with the test manifest's `abc1234`.
const MATCHING_COMMIT: &str = "abc12340000000000000000000000000000000ab";

/// A full 40-hex commit SHA that does not share the manifest's prefix.
const MISMATCHED_COMMIT: &str = "deadbeef00000000000000000000000000000000";

fn commit_sha_strategy() -> impl Strategy<Value = String> {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
    prop::collection::vec(0_u8..16, 40).prop_map(|nibbles| {
        nibbles
            .into_iter()
            .map(|nibble| char::from(HEX_DIGITS[usize::from(nibble)]))
            .collect()
    })
}

fn manifest_with_git_sha(git_sha: &str, sha256: &str) -> serde_json::Result<Manifest> {
    serde_json::from_value(serde_json::json!({
        "git_sha": git_sha,
        "schema_version": 1,
        "toolchain": TOOLCHAIN,
        "target": TARGET,
        "generated_at": "2026-02-03T00:00:00Z",
        "files": ["libwhitaker_suite.so"],
        "sha256": sha256,
    }))
}

/// Writes a test file relative to a capability rooted at its parent directory.
fn write_test_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "test file path has no parent directory",
        )
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "test file path has no file name",
        )
    })?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
    dir.write(file_name, contents)
}

proptest! {
    #[test]
    fn equal_full_git_shas_are_accepted(
        commit in commit_sha_strategy(),
    ) {
        let expected = CommitSha::try_from(commit.as_str())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let manifest = manifest_with_git_sha(&commit, &"a".repeat(64))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert!(validate_git_sha(&manifest, Some(&expected)).is_ok());
    }

    #[test]
    fn abbreviated_manifest_git_shas_are_rejected_for_pinned_installs(
        commit in commit_sha_strategy(),
        prefix_len in 7_usize..40,
    ) {
        let expected = CommitSha::try_from(commit.as_str())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let manifest_sha = &commit[..prefix_len];
        let manifest = manifest_with_git_sha(manifest_sha, &"a".repeat(64))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        match validate_git_sha(&manifest, Some(&expected)) {
            Err(PrebuiltError::GitShaMismatch { manifest, expected }) => {
                prop_assert_eq!(manifest, manifest_sha);
                prop_assert_eq!(expected, commit);
            }
            other => {
                return Err(TestCaseError::fail(format!(
                    "expected GitShaMismatch, got {other:?}"
                )));
            }
        }
    }

    #[test]
    fn distinct_full_git_shas_sharing_a_prefix_are_rejected(
        commit in commit_sha_strategy(),
        shared_prefix_len in 7_usize..40,
    ) {
        let expected = CommitSha::try_from(commit.as_str())
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let mut manifest_sha = commit.clone();
        let replacement = if manifest_sha.as_bytes()[shared_prefix_len] == b'0' {
            "1"
        } else {
            "0"
        };
        manifest_sha.replace_range(shared_prefix_len..=shared_prefix_len, replacement);
        let manifest = manifest_with_git_sha(&manifest_sha, &"a".repeat(64))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        match validate_git_sha(&manifest, Some(&expected)) {
            Err(PrebuiltError::GitShaMismatch { manifest, expected }) => {
                prop_assert_eq!(manifest, manifest_sha);
                prop_assert_eq!(expected, commit);
            }
            other => {
                return Err(TestCaseError::fail(format!(
                    "expected GitShaMismatch, got {other:?}"
                )));
            }
        }
    }
}

fn base_config(destination_dir: &Utf8Path) -> PrebuiltConfig<'_> {
    PrebuiltConfig {
        target: TARGET,
        toolchain: TOOLCHAIN,
        destination_dir,
        quiet: true,
        expected_git_sha: None,
    }
}

/// Construct downloader and extractor mocks for the successful prebuilt path.
fn success_mocks() -> (MockArtefactDownloader, MockArtefactExtractor) {
    success_mocks_with_git_sha("abc1234")
}

/// Construct successful mocks with the supplied manifest provenance.
fn success_mocks_with_git_sha(git_sha: &str) -> (MockArtefactDownloader, MockArtefactExtractor) {
    let fake_sha = sha256_hex(FAKE_ARCHIVE);
    let manifest =
        manifest_with_git_sha(git_sha, &fake_sha).expect("construct manifest with Git SHA");
    let manifest_json = serde_json::to_string(&manifest).expect("serialize manifest with Git SHA");
    let mut downloader = MockArtefactDownloader::new();
    downloader
        .expect_download_manifest()
        .returning(move |_| Ok(manifest_json.clone()));
    downloader
        .expect_download_archive()
        .returning(|_filename, dest| {
            write_test_file(dest, FAKE_ARCHIVE).map_err(DownloadError::Io)
        });
    let mut extractor = MockArtefactExtractor::new();
    extractor.expect_extract().returning(|_archive, dest| {
        let source_name = "libwhitaker_suite.so".to_owned();
        write_test_file(&dest.join(&source_name), b"fake")?;
        Ok(vec![source_name])
    });
    (downloader, extractor)
}

#[rstest]
#[case::mismatch(Some(MISMATCHED_COMMIT), "abc1234", false)]
#[case::matching_full_sha(Some(MATCHING_COMMIT), MATCHING_COMMIT, true)]
#[case::unpinned_abbreviated_sha(None, "abc1234", true)]
fn prebuilt_git_sha_gate(
    #[case] expected_git_sha: Option<&str>,
    #[case] manifest_git_sha: &str,
    #[case] expects_success: bool,
) {
    let (_temp, destination_dir) = destination_dir();
    let expected_commit = expected_git_sha
        .map(CommitSha::try_from)
        .transpose()
        .expect("full test commit SHA");
    let config = PrebuiltConfig {
        expected_git_sha: expected_commit.as_ref(),
        ..base_config(&destination_dir)
    };
    let (downloader, extractor) = success_mocks_with_git_sha(manifest_git_sha);
    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);

    if expects_success {
        assert!(
            matches!(result, PrebuiltResult::Success { .. }),
            "expected Success, got {result:?}"
        );
    } else {
        match result {
            PrebuiltResult::Fallback { reason } => {
                assert!(reason.contains("SHA mismatch"), "reason: {reason}");
            }
            other => panic!("expected Fallback, got {other:?}"),
        }
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
    let (downloader, extractor) = success_mocks();

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

#[path = "prebuilt_provenance_tests.rs"]
mod prebuilt_provenance_tests;
