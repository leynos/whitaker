//! Artefact download logic for prebuilt lint library retrieval.
//!
//! Provides a trait-based abstraction for downloading artefact archives
//! and manifests from the GitHub rolling release, enabling dependency
//! injection for testing.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

/// The GitHub repository owner/name for URL construction.
const GITHUB_REPO: &str = "leynos/whitaker";

/// The rolling release tag name.
const ROLLING_TAG: &str = "rolling";
/// Network timeout for prebuilt artefact downloads.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// Trait for downloading artefact files from a release.
///
/// Abstractions allow tests to mock HTTP behaviour without network access.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::download::HttpDownloader;
///
/// let downloader = HttpDownloader;
/// // Use downloader.download_manifest("x86_64-unknown-linux-gnu") in production
/// ```
#[cfg_attr(test, mockall::automock)]
pub trait ArtefactDownloader {
    /// Download the manifest JSON for the given target triple.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails or the asset is not found.
    fn download_manifest(&self, target: &str) -> Result<String, DownloadError>;

    /// Download the archive for the given filename into `dest`.
    ///
    /// # Errors
    ///
    /// Returns an error if the download or file write fails.
    fn download_archive(&self, filename: &str, dest: &Path) -> Result<(), DownloadError>;
}

/// Errors arising from artefact download operations.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// HTTP request failed.
    #[error("download failed for {url}: {reason}")]
    HttpError {
        /// The URL that was requested.
        url: String,
        /// A human-readable description of the failure.
        reason: String,
    },

    /// The requested artefact was not found (HTTP 404).
    #[error("artefact not found: {url}")]
    NotFound {
        /// The URL that returned 404.
        url: String,
    },

    /// I/O error writing the downloaded file.
    #[error("I/O error writing download: {0}")]
    Io(#[from] std::io::Error),
}

/// HTTP-based downloader using `ureq`.
pub struct HttpDownloader;

impl HttpDownloader {
    /// Construct the GitHub release asset URL for a given filename.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::download::HttpDownloader;
    ///
    /// let url = HttpDownloader::asset_url("archive.tar.zst");
    /// assert!(url.contains("leynos/whitaker"));
    /// assert!(url.contains("rolling"));
    /// ```
    #[must_use]
    pub fn asset_url(filename: &str) -> String {
        format!("https://github.com/{GITHUB_REPO}/releases/download/{ROLLING_TAG}/{filename}")
    }
}

impl ArtefactDownloader for HttpDownloader {
    fn download_manifest(&self, target: &str) -> Result<String, DownloadError> {
        let filename = format!("manifest-{target}.json");
        let url = Self::asset_url(&filename);
        download_text(&url)
    }

    fn download_archive(&self, filename: &str, dest: &Path) -> Result<(), DownloadError> {
        let url = Self::asset_url(filename);
        download_to_file(&url, dest)
    }
}

/// Download a URL and return the body as a string.
fn download_text(url: &str) -> Result<String, DownloadError> {
    let response = http_agent()
        .get(url)
        .call()
        .map_err(|e| map_ureq_error(url, &e))?;
    response
        .into_body()
        .read_to_string()
        .map_err(|e| DownloadError::HttpError {
            url: url.to_owned(),
            reason: e.to_string(),
        })
}

/// Download a URL and write the body to a file.
fn download_to_file(url: &str, dest: &Path) -> Result<(), DownloadError> {
    let response = http_agent()
        .get(url)
        .call()
        .map_err(|e| map_ureq_error(url, &e))?;
    let mut file = std::fs::File::create(dest)?;
    std::io::copy(&mut response.into_body().as_reader(), &mut file).map_err(DownloadError::Io)?;
    Ok(())
}

/// Shared `ureq` agent with request timeout configuration.
fn http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(DOWNLOAD_TIMEOUT))
            .build();
        ureq::Agent::new_with_config(config)
    })
}

/// Map a ureq error to a [`DownloadError`].
fn map_ureq_error(url: &str, err: &ureq::Error) -> DownloadError {
    match err {
        ureq::Error::StatusCode(404) => DownloadError::NotFound {
            url: url.to_owned(),
        },
        other => DownloadError::HttpError {
            url: url.to_owned(),
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_url_contains_repo_and_tag() {
        let url = HttpDownloader::asset_url("test.tar.zst");
        assert!(url.contains(GITHUB_REPO));
        assert!(url.contains(ROLLING_TAG));
        assert!(url.ends_with("test.tar.zst"));
    }

    #[test]
    fn asset_url_for_manifest() {
        let url = HttpDownloader::asset_url("manifest-x86_64-unknown-linux-gnu.json");
        assert!(url.ends_with("manifest-x86_64-unknown-linux-gnu.json"));
    }

    #[test]
    fn map_ureq_error_maps_404_to_not_found() {
        let err = ureq::Error::StatusCode(404);
        let mapped = map_ureq_error("https://example.test/manifest", &err);
        assert!(matches!(mapped, DownloadError::NotFound { .. }));
    }

    #[test]
    fn map_ureq_error_maps_other_status_to_http_error() {
        let err = ureq::Error::StatusCode(500);
        let mapped = map_ureq_error("https://example.test/manifest", &err);
        assert!(matches!(mapped, DownloadError::HttpError { .. }));
    }
}
