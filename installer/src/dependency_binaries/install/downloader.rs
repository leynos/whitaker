//! Download support for repository-hosted dependency-binary archives.

use crate::artefact::download::HttpDownloader;

use super::installer::DependencyBinaryInstallError;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io;
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

        // Compute actual checksum
        let mut archive_file = File::open(destination)?;
        let mut hasher = Sha256::new();
        io::copy(&mut archive_file, &mut hasher)?;
        let actual_checksum = format!("{:x}", hasher.finalize());

        // Verify checksum
        if actual_checksum != expected_checksum {
            return Err(DependencyBinaryInstallError::Checksum {
                archive: destination.to_path_buf(),
                expected: expected_checksum.to_string(),
                actual: actual_checksum,
            });
        }

        Ok(())
    }
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
    //! Tests for downloader error mapping.

    use super::*;

    #[test]
    fn map_ureq_error_maps_missing_asset_statuses_to_not_found() {
        for status in [404, 410] {
            let error = map_ureq_error(
                "https://example.test/archive.tgz",
                &ureq::Error::StatusCode(status),
            );

            assert!(matches!(
                error,
                DependencyBinaryInstallError::NotFound { .. }
            ));
        }
    }

    #[test]
    fn map_ureq_error_maps_non_missing_4xx_to_download_error() {
        let error = map_ureq_error(
            "https://example.test/archive.tgz",
            &ureq::Error::StatusCode(403),
        );

        assert!(matches!(
            error,
            DependencyBinaryInstallError::Download { .. }
        ));
    }

    #[test]
    fn map_ureq_error_maps_other_status_to_download_error() {
        let error = map_ureq_error(
            "https://example.test/archive.tgz",
            &ureq::Error::StatusCode(500),
        );

        assert!(matches!(
            error,
            DependencyBinaryInstallError::Download { .. }
        ));
    }
}
