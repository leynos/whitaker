use super::*;
use crate::artefact::download::MockArtefactDownloader;
use crate::artefact::extraction::MockArtefactExtractor;

/// Build a manifest JSON string with a configurable toolchain and SHA-256.
fn manifest_json_with(toolchain: &str, sha256: &str) -> String {
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

/// Compute the SHA-256 of a byte slice (for test fixtures).
fn sha256_of(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(data))
}

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

#[test]
fn happy_path_returns_success() {
    let (_temp, staging_base) = staging_base();
    let config = base_config(&staging_base);
    let fake_sha = sha256_of(FAKE_ARCHIVE);
    let manifest_json = manifest_json_with(TOOLCHAIN, &fake_sha);

    let mut downloader = MockArtefactDownloader::new();
    downloader
        .expect_download_manifest()
        .returning(move |_| Ok(manifest_json.clone()));
    downloader
        .expect_download_archive()
        .returning(|_filename, dest| std::fs::write(dest, FAKE_ARCHIVE).map_err(DownloadError::Io));

    let mut extractor = MockArtefactExtractor::new();
    extractor
        .expect_extract()
        .returning(|_archive, _dest| Ok(vec!["lib.so".to_owned()]));

    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    assert!(
        matches!(result, PrebuiltResult::Success { .. }),
        "expected Success, got {result:?}"
    );
}

#[test]
fn download_failure_returns_fallback() {
    let (_temp, staging_base) = staging_base();
    let config = base_config(&staging_base);

    let mut downloader = MockArtefactDownloader::new();
    downloader.expect_download_manifest().returning(|_| {
        Err(DownloadError::HttpError {
            url: "http://example.com".to_owned(),
            reason: "connection refused".to_owned(),
        })
    });

    let extractor = MockArtefactExtractor::new();
    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    match result {
        PrebuiltResult::Fallback { reason } => {
            assert!(reason.contains("download"), "reason: {reason}");
        }
        other => panic!("expected Fallback, got {other:?}"),
    }
}

#[test]
fn not_found_returns_fallback() {
    let (_temp, staging_base) = staging_base();
    let config = base_config(&staging_base);

    let mut downloader = MockArtefactDownloader::new();
    downloader.expect_download_manifest().returning(|_| {
        Err(DownloadError::NotFound {
            url: "http://example.com/manifest".to_owned(),
        })
    });

    let extractor = MockArtefactExtractor::new();
    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    match result {
        PrebuiltResult::Fallback { reason } => {
            assert!(reason.contains("not found"), "reason: {reason}");
        }
        other => panic!("expected Fallback, got {other:?}"),
    }
}

#[test]
fn toolchain_mismatch_returns_fallback() {
    let (_temp, staging_base) = staging_base();
    let config = base_config(&staging_base);

    let mut downloader = MockArtefactDownloader::new();
    let manifest_json = manifest_json_with("nightly-2025-01-01", &"a".repeat(64));
    downloader
        .expect_download_manifest()
        .returning(move |_| Ok(manifest_json.clone()));

    let extractor = MockArtefactExtractor::new();
    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    match result {
        PrebuiltResult::Fallback { reason } => {
            assert!(reason.contains("toolchain mismatch"), "reason: {reason}");
        }
        other => panic!("expected Fallback, got {other:?}"),
    }
}

#[test]
fn checksum_mismatch_returns_fallback() {
    let (_temp, staging_base) = staging_base();
    let config = base_config(&staging_base);

    // Manifest claims SHA = "aaa...a" but the file will hash differently.
    let mut downloader = MockArtefactDownloader::new();
    let manifest_json = manifest_json_with(TOOLCHAIN, &"a".repeat(64));
    downloader
        .expect_download_manifest()
        .returning(move |_| Ok(manifest_json.clone()));
    downloader
        .expect_download_archive()
        .returning(|_filename, dest| {
            std::fs::write(dest, b"wrong content").map_err(DownloadError::Io)
        });

    let extractor = MockArtefactExtractor::new();
    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    match result {
        PrebuiltResult::Fallback { reason } => {
            assert!(reason.contains("checksum mismatch"), "reason: {reason}");
        }
        other => panic!("expected Fallback, got {other:?}"),
    }
}

#[test]
fn extraction_failure_returns_fallback() {
    let (_temp, staging_base) = staging_base();
    let config = base_config(&staging_base);
    let fake_sha = sha256_of(FAKE_ARCHIVE);

    let mut downloader = MockArtefactDownloader::new();
    let manifest_json = manifest_json_with(TOOLCHAIN, &fake_sha);
    downloader
        .expect_download_manifest()
        .returning(move |_| Ok(manifest_json.clone()));
    downloader
        .expect_download_archive()
        .returning(|_filename, dest| std::fs::write(dest, FAKE_ARCHIVE).map_err(DownloadError::Io));

    let mut extractor = MockArtefactExtractor::new();
    extractor.expect_extract().returning(|_archive, _dest| {
        Err(crate::artefact::extraction::ExtractionError::EmptyArchive)
    });

    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);
    match result {
        PrebuiltResult::Fallback { reason } => {
            assert!(reason.contains("extraction"), "reason: {reason}");
        }
        other => panic!("expected Fallback, got {other:?}"),
    }
}
