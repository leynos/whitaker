//! Download support for repository-hosted dependency-binary archives.

use crate::artefact::download::HttpDownloader;

use super::installer::DependencyBinaryInstallError;
use crate::hex::to_lower_hex;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;

const DOWNLOAD_TIMEOUT_SECS: u64 = 30;

/// Downloads dependency archives.
#[cfg_attr(test, mockall::automock)]
pub trait DependencyArchiveDownloader {
    /// Download `filename` into `destination` and verify its SHA-256 checksum.
    ///
    /// # Errors
    ///
    /// Returns an error when the remote asset cannot be fetched or checksum
    /// verification fails.
    fn download(
        &self,
        filename: &str,
        destination: &Path,
    ) -> Result<(), DependencyBinaryInstallError>;
}

/// Production downloader for release archives.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RepositoryArchiveDownloader;

impl DependencyArchiveDownloader for RepositoryArchiveDownloader {
    fn download(
        &self,
        filename: &str,
        destination: &Path,
    ) -> Result<(), DependencyBinaryInstallError> {
        let url = asset_url(filename);
        let checksum_url = format!("{url}.sha256");
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS)))
            .build();
        let agent = ureq::Agent::new_with_config(config);

        // Download the archive
        let response = agent
            .get(&url)
            .call()
            .map_err(|error| map_ureq_error(&url, &error))?;
        let mut file = File::create(destination)?;
        let mut body = response.into_body();
        let mut reader = body.as_reader();
        io::copy(&mut reader, &mut file)?;
        drop(file);

        // Download and parse the expected checksum
        let checksum_response = agent
            .get(&checksum_url)
            .call()
            .map_err(|error| map_ureq_error(&checksum_url, &error))?;
        let checksum_body = checksum_response
            .into_body()
            .read_to_string()
            .map_err(|error| DependencyBinaryInstallError::Download {
                url: checksum_url.clone(),
                reason: error.to_string(),
            })?;
        let expected_checksum = checksum_body
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().next())
            .ok_or_else(|| DependencyBinaryInstallError::Download {
                url: checksum_url.clone(),
                reason: "empty or invalid checksum file".to_string(),
            })?;

        // Verify the archive against the expected checksum.
        verify_archive_checksum(destination, expected_checksum)
    }
}

/// Compute the lowercase-hex SHA-256 digest of the file at `path`.
///
/// Reads the file in fixed-size chunks so archives of any size hash with a
/// bounded buffer.
fn compute_file_sha256(path: &Path) -> io::Result<String> {
    let mut archive_file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = archive_file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(to_lower_hex(&hasher.finalize()))
}

/// Verify that the archive at `destination` matches the `expected` digest.
///
/// # Errors
///
/// Returns [`DependencyBinaryInstallError::Checksum`] when the computed digest
/// differs from `expected`, and propagates any I/O error encountered while
/// reading the archive.
fn verify_archive_checksum(
    destination: &Path,
    expected: &str,
) -> Result<(), DependencyBinaryInstallError> {
    let actual_checksum = compute_file_sha256(destination)?;
    if actual_checksum != expected {
        return Err(DependencyBinaryInstallError::Checksum {
            archive: destination.to_path_buf(),
            expected: expected.to_string(),
            actual: actual_checksum,
        });
    }
    Ok(())
}

/// Build the rolling-release asset URL for one dependency archive filename.
fn asset_url(filename: &str) -> String {
    // Dependency binaries are published to the rolling release so the
    // repository-owned manifest can advance independently of installer tags.
    HttpDownloader::asset_url(filename)
}

/// Map `ureq` failures into semantic dependency-installer errors.
fn map_ureq_error(url: &str, error: &ureq::Error) -> DependencyBinaryInstallError {
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

#[cfg(test)]
mod tests {
    //! Tests for downloader error mapping and archive checksum verification.

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
    fn compute_file_sha256_matches_known_vector() {
        let file = temp_file_with(b"abc");
        assert_eq!(
            compute_file_sha256(file.path()).expect("hash temp file"),
            concat!(
                "ba7816bf8f01cfea414140de5dae2223",
                "b00361a396177a9cb410ff61f20015ad",
            ),
        );
    }

    #[test]
    fn compute_file_sha256_hashes_content_larger_than_the_buffer() {
        // Exercise the buffered read loop across several 8192-byte reads: the
        // chunked digest must equal a single-shot digest of the same bytes.
        let payload = vec![0xa5_u8; 8192 * 3 + 17];
        let file = temp_file_with(&payload);
        assert_eq!(
            compute_file_sha256(file.path()).expect("hash temp file"),
            to_lower_hex(&Sha256::digest(&payload)),
        );
    }

    #[test]
    fn verify_archive_checksum_accepts_a_matching_digest() {
        let file = temp_file_with(b"hello world");
        let expected = compute_file_sha256(file.path()).expect("hash temp file");
        assert!(verify_archive_checksum(file.path(), &expected).is_ok());
    }

    #[test]
    fn verify_archive_checksum_rejects_a_mismatched_digest() {
        let file = temp_file_with(b"hello world");
        let wrong = "0".repeat(64);
        let error = verify_archive_checksum(file.path(), &wrong)
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
