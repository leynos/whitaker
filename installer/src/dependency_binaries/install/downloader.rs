//! Download support for repository-hosted dependency-binary archives.

use crate::artefact::download::HttpDownloader;

use super::checksum::{fetch_expected_checksum, verify_archive_checksum};
use super::installer::DependencyBinaryInstallError;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use std::io;
use std::path::Path;
use tracing::{debug, instrument, warn};

const DOWNLOAD_TIMEOUT_SECS: u64 = 30;

// Bounded `category` field emitted on every boundary event, kept stable so
// operators can aggregate download failures without unbounded label
// cardinality: `utf8` (non-UTF-8 destination), `capability` (a `cap_std`
// directory create/open/reopen), `fetch` (a network request), `write`
// (streaming the archive to disk), and `checksum` (checksum retrieval, parse,
// or verification — shared with `super::checksum`).
const CATEGORY_UTF8: &str = "utf8";
const CATEGORY_CAPABILITY: &str = "capability";
const CATEGORY_FETCH: &str = "fetch";
const CATEGORY_WRITE: &str = "write";
// Shared with `super::checksum`, which emits the same category on its own
// checksum boundary events.
pub(super) const CATEGORY_CHECKSUM: &str = "checksum";

// Bounded `checksum_state` field values marking the checksum-processing stage
// the orchestrator reports (the `parsed` state is emitted by `super::checksum`).
const CHECKSUM_STATE_MISMATCH: &str = "mismatch";
const CHECKSUM_STATE_VERIFIED: &str = "verified";

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
        let archive_url = HttpDownloader::asset_url(filename);
        let checksum_url = format!("{archive_url}.sha256");
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS)))
            .build();
        let agent = ureq::Agent::new_with_config(config);
        download_from_urls(&agent, &archive_url, &checksum_url, destination)
    }
}

/// Run the download workflow against explicit archive and checksum URLs.
///
/// This internal seam keeps [`RepositoryArchiveDownloader::download`] as
/// production orchestration — agent construction and release-URL derivation —
/// while letting tests drive the full boundary sequence against a local server.
/// The public API exposes no URL override.
///
/// The destination is validated and its parent-directory capability is opened
/// before the first HTTP request, so an invalid destination fails without any
/// network access.
///
/// # Errors
///
/// Propagates every [`DependencyBinaryInstallError`] raised by destination
/// validation, archive download, checksum retrieval, and verification.
#[instrument(
    name = "dependency_archive_download",
    skip(agent),
    fields(
        archive_url = %archive_url,
        checksum_url = %checksum_url,
        destination = %destination.display(),
    ),
)]
pub(super) fn download_from_urls(
    agent: &ureq::Agent,
    archive_url: &str,
    checksum_url: &str,
    destination: &Path,
) -> Result<(), DependencyBinaryInstallError> {
    debug!("starting dependency archive download");

    // Acquire a parent-directory capability up front; every archive read and
    // write flows through it, so the downloader never reaches for ambient
    // `std::fs` file access. Validation happens here, before any HTTP request.
    let (destination, dir, archive_name) = open_download_destination(destination)?;
    download_archive(agent, archive_url, &dir, &archive_name)?;
    let expected_checksum = fetch_expected_checksum(agent, checksum_url)?;

    // Re-open the freshly written archive through the same capability and verify
    // it; the checksum helper stays pure over the reader. `verify_archive_checksum`
    // consumes the reader, so the handle is closed before any cleanup below.
    let archive = dir.open(&archive_name).inspect_err(|error| {
        warn!(
            category = CATEGORY_CAPABILITY,
            archive_name = %archive_name,
            error = %error,
            "failed to reopen archive for verification",
        );
    })?;
    match verify_archive_checksum(archive, destination.as_std_path(), &expected_checksum) {
        Ok(()) => {
            debug!(
                category = CATEGORY_CHECKSUM,
                checksum_state = CHECKSUM_STATE_VERIFIED,
                "archive checksum verified",
            );
            Ok(())
        }
        Err(error) => {
            warn!(
                category = CATEGORY_CHECKSUM,
                checksum_state = CHECKSUM_STATE_MISMATCH,
                url = %archive_url,
                error = %error,
                "archive checksum verification failed",
            );
            // Remove the unverified archive through the same capability so a
            // retry never observes stale, unverified data at the destination.
            if let Err(cleanup_error) = dir.remove_file(&archive_name) {
                warn!(
                    category = CATEGORY_CAPABILITY,
                    archive_name = %archive_name,
                    error = %cleanup_error,
                    "failed to remove unverified archive after checksum failure",
                );
            }
            Err(error)
        }
    }
}

/// Validate `destination` as UTF-8 and open its parent directory as a
/// capability, returning owned values so the caller has no borrowed-lifetime
/// coupling to the directory handle or archive name.
///
/// # Errors
///
/// Returns [`DependencyBinaryInstallError::Io`] with
/// [`io::ErrorKind::InvalidInput`] when `destination` is not valid UTF-8, and
/// propagates capability-open failures from [`open_destination_dir`].
fn open_download_destination(
    destination: &Path,
) -> Result<(Utf8PathBuf, Dir, String), DependencyBinaryInstallError> {
    let destination = Utf8Path::from_path(destination).ok_or_else(|| {
        warn!(
            category = CATEGORY_UTF8,
            "destination archive path is not valid UTF-8",
        );
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination archive path is not valid UTF-8",
        )
    })?;
    let (dir, archive_name) = open_destination_dir(destination).inspect_err(|error| {
        warn!(
            category = CATEGORY_CAPABILITY,
            destination = %destination,
            error = %error,
            "failed to open destination directory",
        );
    })?;
    Ok((destination.to_owned(), dir, archive_name.to_owned()))
}

/// Fetch the archive at `url` and write it into `dir` as `archive_name`.
///
/// The response body is streamed only into the handle returned by
/// `dir.create`, which is dropped before returning.
///
/// # Errors
///
/// Returns a mapped [`map_ureq_error`] failure when the fetch fails, and
/// propagates capability-create and write failures.
fn download_archive(
    agent: &ureq::Agent,
    url: &str,
    dir: &Dir,
    archive_name: &str,
) -> Result<(), DependencyBinaryInstallError> {
    let response = agent
        .get(url)
        .call()
        .map_err(|error| map_ureq_error(url, &error))
        .inspect_err(|error| {
            warn!(
                category = CATEGORY_FETCH,
                url = %url,
                error = %error,
                "archive fetch failed",
            );
        })?;
    let mut file = dir.create(archive_name).inspect_err(|error| {
        warn!(
            category = CATEGORY_CAPABILITY,
            archive_name = %archive_name,
            error = %error,
            "failed to create archive file",
        );
    })?;
    let mut body = response.into_body();
    let mut reader = body.as_reader();
    io::copy(&mut reader, &mut file).inspect_err(|error| {
        warn!(
            category = CATEGORY_WRITE,
            url = %url,
            error = %error,
            "failed to write archive to disk",
        );
    })?;
    drop(file);
    Ok(())
}

/// Open the parent directory of `destination` as a capability, returning it
/// alongside the archive's file name.
///
/// `cap_std` grants no ambient authority, so the parent directory is opened
/// explicitly; all subsequent archive I/O is scoped to the returned handle
/// rather than routed through ambient `std::fs`.
///
/// # Errors
///
/// Returns an I/O error when `destination` has no file name or its parent
/// directory cannot be opened.
fn open_destination_dir(destination: &Utf8Path) -> io::Result<(Dir, &str)> {
    let parent = match destination.parent() {
        Some(parent) if !parent.as_str().is_empty() => parent,
        _ => Utf8Path::new("."),
    };
    let archive_name = destination.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination archive path has no file name",
        )
    })?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
    Ok((dir, archive_name))
}

/// Map `ureq` failures into semantic dependency-installer errors.
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

#[cfg(test)]
mod tests {
    //! Tests for downloader error mapping and archive checksum verification.

    use super::*;
    use crate::hex::to_lower_hex;
    use rstest::rstest;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use tempfile::TempDir;

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
    fn open_destination_dir_rejects_a_path_without_a_file_name() {
        // The filesystem root has no file name, so the capability boundary
        // cannot derive an archive name and must reject it up front.
        let root = Utf8Path::new("/");
        let error = open_destination_dir(root).expect_err("root path has no file name");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("has no file name"),
            "error should identify the missing file name, got: {error}",
        );
    }

    /// Removes a probe file from a capability-scoped directory when dropped, so
    /// a leaked artefact is cleaned up even if an assertion panics first.
    struct ProbeCleanup {
        dir: Dir,
        name: String,
    }

    impl Drop for ProbeCleanup {
        fn drop(&mut self) {
            // A correct run never writes the probe into this directory, so a
            // missing file is the expected case.
            let _ = self.dir.remove_file(&self.name);
        }
    }

    #[test]
    fn open_destination_dir_writes_into_the_destination_parent_not_the_cwd() {
        let temp = TempDir::new().expect("create temp dir");
        let temp_dir = Utf8Path::from_path(temp.path()).expect("temp path is UTF-8");
        // A unique, test-owned probe name derived from the temp directory, so
        // it can never collide with — or delete — a pre-existing file in the
        // working directory.
        let archive_file = format!(
            "{}.tgz",
            temp_dir.file_name().expect("temp dir has a file name"),
        );
        let destination = temp_dir.join(&archive_file);

        // Open the process working directory as a capability, paired with RAII
        // cleanup: if a regression leaks the probe here, it is removed on drop
        // even when an assertion below panics first.
        let cwd_probe = ProbeCleanup {
            dir: Dir::open_ambient_dir(".", ambient_authority()).expect("open cwd capability"),
            name: archive_file.clone(),
        };
        // An independent capability for the destination's directory, used to
        // confirm the archive actually lands there rather than trusting the
        // directory handle returned by the code under test.
        let destination_dir = Dir::open_ambient_dir(temp_dir, ambient_authority())
            .expect("open destination capability");

        let (dir, archive_name) = open_destination_dir(&destination).expect("open destination dir");
        assert_eq!(archive_name, archive_file.as_str());

        let mut file = dir
            .create(archive_name)
            .expect("create archive via capability");
        file.write_all(b"hello world").expect("write archive");
        drop(file);

        // The capability must write into the destination's parent directory...
        assert!(
            destination_dir.exists(&archive_file),
            "archive must exist at the destination path",
        );
        // ...and never into the process working directory. This assertion fails
        // if `open_destination_dir` opens `.` for a destination with a real
        // parent.
        assert!(
            !cwd_probe.dir.exists(&archive_file),
            "capability must not create the archive in the current working directory",
        );

        // Re-open through the same capability and keep the end-to-end checksum
        // assertion.
        let expected = to_lower_hex(&Sha256::digest(b"hello world"));
        let archive = dir.open(archive_name).expect("open archive via capability");
        assert!(verify_archive_checksum(archive, destination.as_std_path(), &expected).is_ok());

        // `cwd_probe` drops here (or on any earlier panic), removing a leaked
        // probe through its capability.
        drop(cwd_probe);
    }
}
