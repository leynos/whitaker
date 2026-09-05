//! Regression coverage for pinned prebuilt provenance gating.

use super::*;
use crate::artefact::download::MockArtefactDownloader;
use crate::artefact::extraction::MockArtefactExtractor;
use crate::git::CommitSha;
use crate::test_utils::sha256_hex;

#[test]
fn mismatched_pinned_manifest_skips_archive_download() {
    let (_temp, destination_dir) = destination_dir();
    let expected = CommitSha::try_from(MATCHING_COMMIT).expect("full test commit SHA");
    let config = PrebuiltConfig {
        expected_git_sha: Some(&expected),
        ..base_config(&destination_dir)
    };
    let fake_sha = sha256_hex(FAKE_ARCHIVE);
    let manifest =
        manifest_with_git_sha(MISMATCHED_COMMIT, &fake_sha).expect("construct mismatched manifest");
    let manifest_json = serde_json::to_string(&manifest).expect("serialize mismatched manifest");
    let mut downloader = MockArtefactDownloader::new();
    downloader
        .expect_download_manifest()
        .returning(move |_| Ok(manifest_json.clone()));
    downloader.expect_download_archive().times(0);
    let mut extractor = MockArtefactExtractor::new();
    extractor.expect_extract().times(0);

    let mut stderr = Vec::new();
    let result = attempt_prebuilt_with(&config, &downloader, &extractor, &mut stderr);

    assert!(
        matches!(result, PrebuiltResult::Fallback { ref reason } if reason.contains("SHA mismatch")),
        "a mismatched pinned manifest must fail before archive download, got {result:?}"
    );
}
