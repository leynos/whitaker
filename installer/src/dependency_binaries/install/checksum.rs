//! Checksum retrieval and SHA-256 verification for dependency archives.
//!
//! These helpers fetch the `.sha256` sidecar, parse its digest token, stream a
//! bounded SHA-256 over an archive reader, and compare the two. They are kept
//! separate from the download orchestration in [`super::downloader`] so each
//! module stays focused. The bounded [`CATEGORY_CHECKSUM`] tracing category is
//! owned here and imported by `downloader`, keeping the module dependency
//! one-way.

use super::installer::DependencyBinaryInstallError;
use crate::hex::to_lower_hex;
use sha2::{Digest, Sha256};
use std::io;
use std::io::Read;
use std::path::Path;
use tracing::{debug, warn};

/// Bounded `category` field for every checksum boundary event. Owned here so the
/// module dependency stays one-way (`downloader` imports this; not vice versa).
pub(super) const CATEGORY_CHECKSUM: &str = "checksum";

// Bounded `checksum_state` field value marking a successfully parsed digest.
const CHECKSUM_STATE_PARSED: &str = "parsed";

/// Maximum `.sha256` sidecar size read into memory. A sidecar line is a 64-hex
/// digest plus a file name (~80 bytes), so this leaves ample headroom while
/// tightening `ureq`'s 10 MiB default well below anything that could pressure
/// memory.
const CHECKSUM_MAX_BYTES: u64 = 64 * 1024;

/// Map `ureq` failures into semantic dependency-installer errors. Shared with
/// `super::downloader`, which maps archive-fetch failures the same way.
pub(super) fn map_ureq_error(url: &str, error: &ureq::Error) -> DependencyBinaryInstallError {
    match error {
        ureq::Error::StatusCode(404 | 410) => DependencyBinaryInstallError::NotFound {
            url: url.to_owned(),
        },
        other => DependencyBinaryInstallError::Download {
            url: url.to_owned(),
            reason: other.to_string(),
        },
    }
}

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
        .into_with_config()
        .limit(CHECKSUM_MAX_BYTES)
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

/// Extract the SHA-256 digest token (first whitespace-delimited field of the
/// first line) from a checksum-file body, returning `None` unless it is exactly
/// 64 ASCII hexadecimal digits. This rejects empty, blank, truncated, non-hex,
/// and HTML-error bodies so the caller surfaces its `Download` error.
fn parse_checksum_token(body: &str) -> Option<&str> {
    let token = body.lines().next()?.split_whitespace().next()?;
    (token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(token)
}

/// Compute the lowercase-hex SHA-256 digest of `reader`.
///
/// Reads the stream in fixed-size chunks so inputs of any size hash with a
/// bounded buffer. The caller owns opening and scoping the underlying handle,
/// keeping this a pure transformation over the byte stream. An `Interrupted`
/// read is retried, matching `read_to_end` and `io::copy`.
fn compute_sha256(mut reader: impl Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => bytes_read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
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

    #[rstest]
    #[case(404, true)]
    #[case(410, true)]
    #[case(403, false)]
    #[case(500, false)]
    fn map_ureq_error_maps_status_codes(#[case] status: u16, #[case] is_not_found: bool) {
        let error = map_ureq_error(
            "https://example.test/archive.tgz",
            &ureq::Error::StatusCode(status),
        );

        if is_not_found {
            assert!(matches!(
                error,
                DependencyBinaryInstallError::NotFound { .. }
            ));
        } else {
            assert!(matches!(
                error,
                DependencyBinaryInstallError::Download { .. }
            ));
        }
    }

    #[test]
    fn parse_checksum_token_returns_a_valid_64_hex_digest() {
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let body = format!("{digest}  whitaker-tool.tgz\nignored second line\n");
        assert_eq!(parse_checksum_token(&body), Some(digest));
    }

    #[rstest]
    #[case::empty("")]
    #[case::blank("   \n")]
    #[case::blank_lines("\n\n")]
    #[case::truncated("abc123  whitaker-tool.tgz\n")]
    #[case::non_hex("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz1  x\n")]
    #[case::html("<html><body>404 Not Found</body></html>\n")]
    fn parse_checksum_token_rejects_a_malformed_body(#[case] body: &str) {
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

    /// A reader that yields at most `chunk` bytes per `read` and counts calls,
    /// so a test can prove the stream is consumed in bounded increments.
    struct ChunkedReader<'a> {
        data: &'a [u8],
        chunk: usize,
        reads: usize,
        /// Number of leading reads that fail with `Interrupted` before data flows.
        pending_interrupts: usize,
    }

    impl Read for ChunkedReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            if self.pending_interrupts > 0 {
                self.pending_interrupts -= 1;
                return Err(io::Error::new(io::ErrorKind::Interrupted, "interrupted"));
            }
            let take = self.data.len().min(self.chunk).min(buf.len());
            let (to_copy, rest) = self.data.split_at(take);
            buf.get_mut(..take)
                .expect("take is bounded by buf.len()")
                .copy_from_slice(to_copy);
            self.data = rest;
            Ok(take)
        }
    }

    #[test]
    fn compute_sha256_retries_after_an_interrupted_read() {
        // `Read::read` may return `Interrupted` (EINTR) without any data. The
        // loop must retry rather than surface it, as `read_to_end`/`io::copy` do.
        let mut reader = ChunkedReader {
            data: b"abc",
            chunk: 8192,
            reads: 0,
            pending_interrupts: 1,
        };
        assert_eq!(
            compute_sha256(&mut reader).expect("an interrupted read must be retried"),
            concat!(
                "ba7816bf8f01cfea414140de5dae2223",
                "b00361a396177a9cb410ff61f20015ad",
            ),
        );
    }

    #[test]
    fn compute_sha256_consumes_the_stream_in_bounded_reads() {
        // A payload spanning several 8192-byte buffers, served in small chunks so
        // the hasher must issue many `read` calls; the streamed digest must still
        // equal a single-shot digest of the same bytes.
        let payload = vec![0xa5_u8; 8192 * 3 + 17];
        let mut reader = ChunkedReader {
            data: &payload,
            chunk: 1000,
            reads: 0,
            pending_interrupts: 0,
        };
        let digest = compute_sha256(&mut reader).expect("hash archive stream");
        assert_eq!(digest, to_lower_hex(&Sha256::digest(&payload)));
        assert!(
            reader.reads > 1,
            "expected multiple bounded reads, got {}",
            reader.reads,
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
