//! Download support for repository-hosted dependency-binary archives.

use crate::artefact::download::HttpDownloader;

use super::installer::DependencyBinaryInstallError;
use std::fs::File;
use std::io;
use std::path::Path;

const DOWNLOAD_TIMEOUT_SECS: u64 = 30;

/// Downloads dependency archives.
#[cfg_attr(test, mockall::automock)]
pub trait DependencyArchiveDownloader {
    /// Download `filename` into `destination`.
    ///
    /// # Errors
    ///
    /// Returns an error when the remote asset cannot be fetched.
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
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS)))
            .build();
        let response = ureq::Agent::new_with_config(config)
            .get(&url)
            .call()
            .map_err(|error| DependencyBinaryInstallError::Download {
                url: url.clone(),
                reason: error.to_string(),
            })?;
        let mut file = File::create(destination)?;
        let mut body = response.into_body();
        let mut reader = body.as_reader();
        io::copy(&mut reader, &mut file)?;
        Ok(())
    }
}

/// Build the rolling-release asset URL for one dependency archive filename.
fn asset_url(filename: &str) -> String {
    // Dependency binaries are published to the rolling release so the
    // repository-owned manifest can advance independently of installer tags.
    HttpDownloader::asset_url(filename)
}
