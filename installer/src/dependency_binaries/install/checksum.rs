//! Checksum retrieval and SHA-256 verification for dependency archives.
//!
//! These helpers fetch the `.sha256` sidecar, parse its digest token, stream a
//! bounded SHA-256 over an archive reader, and compare the two. They are kept
//! separate from the download orchestration in [`super::downloader`] so each
//! module stays focused. Checksum tracing reuses the shared bounded
//! [`super::downloader::CATEGORY_CHECKSUM`] category.

use super::downloader::{CATEGORY_CHECKSUM, map_ureq_error};
use super::installer::DependencyBinaryInstallError;
use crate::hex::to_lower_hex;
use sha2::{Digest, Sha256};
use std::io;
use std::io::Read;
use std::path::Path;
use tracing::{debug, warn};

// Bounded `checksum_state` field value marking a successfully parsed digest.
const CHECKSUM_STATE_PARSED: &str = "parsed";

/// Fetch the `.sha256` sidecar at `checksum_url` and return the expected digest.
///
/// The token is lowercased so an upper-case sidecar digest still compares equal
/// to [`compute_sha256`]'s lower-case output.
///
/// # Errors
///
/// Returns [`DependencyBinaryInstallError::Download`] on fetch, body-read, or
/// malformed/empty checksum failures.
pub(super) fn fetch_expected_checksum(
    agent: &ureq::Agent,
    checksum_url: &str,
) -> Result<String, DependencyBinaryInstallError> {
    let checksum_response = agent
        .get(checksum_url)
        .call()
        .map_err(|error| map_ureq_error(checksum_url, &error))
        .inspect_err(|error| {
            warn!(
                category = CATEGORY_CHECKSUM,
                url = %checksum_url,
                error = %error,
                "checksum fetch failed",
            );
        })?;
    let checksum_body = checksum_response
        .into_body()
        .read_to_string()
        .map_err(|error| DependencyBinaryInstallError::Download {
            url: checksum_url.to_owned(),
            reason: error.to_string(),
        })
        .inspect_err(|error| {
            warn!(
                category = CATEGORY_CHECKSUM,
                url = %checksum_url,
                error = %error,
                "checksum read failed",
            );
        })?;
    // Convert the pure parser's `Option` into the URL-bearing failure, then
    // normalize to lower case so an upper-case sidecar digest still matches the
    // lower-case digest produced by `compute_sha256`.
    let token = parse_checksum_token(&checksum_body).ok_or_else(|| {
        warn!(
            category = CATEGORY_CHECKSUM,
            url = %checksum_url,
            "empty or invalid checksum file",
        );
        DependencyBinaryInstallError::Download {
            url: checksum_url.to_owned(),
            reason: "empty or invalid checksum file".to_string(),
        }
    })?;
    let expected = token.to_ascii_lowercase();
    debug!(
        category = CATEGORY_CHECKSUM,
        checksum_state = CHECKSUM_STATE_PARSED,
        url = %checksum_url,
        "parsed expected checksum",
    );
    Ok(expected)
}

/// Extract the digest token (first whitespace-delimited field of the first
/// line) from a checksum-file body, returning `None` for an empty, blank, or
/// otherwise tokenless body.
fn parse_checksum_token(body: &str) -> Option<&str> {
    body.lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
}

/// Compute the lowercase-hex SHA-256 digest of `reader`.
///
/// Reads the stream in fixed-size chunks so inputs of any size hash with a
/// bounded buffer. The caller owns opening and scoping the underlying handle,
/// keeping this a pure transformation over the byte stream.
fn compute_sha256(mut reader: impl Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(to_lower_hex(&hasher.finalize()))
}

/// Verify that `reader` hashes to `expected`, attributing a mismatch to
/// `archive`.
///
/// The caller opens and scopes `reader`; `archive` names the source only for
/// diagnostics. Keeping verification pure over the stream avoids re-opening the
/// archive path here.
///
/// # Errors
///
/// Returns [`DependencyBinaryInstallError::Checksum`] when the computed digest
/// differs from `expected`, and propagates any I/O error encountered while
/// reading the stream.
pub(super) fn verify_archive_checksum(
    reader: impl Read,
    archive: &Path,
    expected: &str,
) -> Result<(), DependencyBinaryInstallError> {
    let actual_checksum = compute_sha256(reader)?;
    if actual_checksum != expected {
        return Err(DependencyBinaryInstallError::Checksum {
            archive: archive.to_path_buf(),
            expected: expected.to_string(),
            actual: actual_checksum,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for checksum parsing, streaming SHA-256, and verification.

    use super::*;
    use rstest::rstest;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Write `contents` to a fresh temp file and return the handle.
    fn temp_file_with(contents: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("create temp file");
        file.write_all(contents).expect("write temp file");
        file.flush().expect("flush temp file");
        file
    }

    /// Reopen `file` as a fresh read handle for streaming into the hasher.
    fn read_handle(file: &NamedTempFile) -> impl Read {
        file.reopen().expect("reopen temp file")
    }

    #[test]
    fn parse_checksum_token_returns_the_first_field_of_the_first_line() {
        let body = "abc123def456  whitaker-tool.tgz\nignored second line\n";
        assert_eq!(parse_checksum_token(body), Some("abc123def456"));
    }

    #[rstest]
    #[case("")]
    #[case("   \n")]
    #[case("\n\n")]
    fn parse_checksum_token_returns_none_for_an_empty_or_blank_body(#[case] body: &str) {
        assert_eq!(parse_checksum_token(body), None);
    }

    #[test]
    fn compute_sha256_matches_known_vector() {
        let file = temp_file_with(b"abc");
        assert_eq!(
            compute_sha256(read_handle(&file)).expect("hash archive stream"),
            concat!(
                "ba7816bf8f01cfea414140de5dae2223",
                "b00361a396177a9cb410ff61f20015ad",
            ),
        );
    }

    #[test]
    fn compute_sha256_hashes_content_larger_than_the_buffer() {
        // Exercise the buffered read loop across several 8192-byte reads: the
        // chunked digest must equal a single-shot digest of the same bytes.
        let payload = vec![0xa5_u8; 8192 * 3 + 17];
        let file = temp_file_with(&payload);
        assert_eq!(
            compute_sha256(read_handle(&file)).expect("hash archive stream"),
            to_lower_hex(&Sha256::digest(&payload)),
        );
    }

    #[test]
    fn verify_archive_checksum_accepts_a_matching_digest() {
        let file = temp_file_with(b"hello world");
        let expected = compute_sha256(read_handle(&file)).expect("hash archive stream");
        assert!(verify_archive_checksum(read_handle(&file), file.path(), &expected).is_ok());
    }

    #[test]
    fn verify_archive_checksum_rejects_a_mismatched_digest() {
        let file = temp_file_with(b"hello world");
        let wrong = "0".repeat(64);
        let error = verify_archive_checksum(read_handle(&file), file.path(), &wrong)
            .expect_err("mismatched checksum must fail");
        match error {
            DependencyBinaryInstallError::Checksum {
                archive,
                expected,
                actual,
            } => {
                assert_eq!(archive, file.path());
                assert_eq!(expected, wrong);
                assert_eq!(actual.len(), 64);
                assert_ne!(actual, wrong);
            }
            other => panic!("expected a Checksum error, got {other:?}"),
        }
    }
}
