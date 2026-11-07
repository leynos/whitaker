//! Download support for repository-hosted dependency-binary archives.

use std::{
    io::{self, Read, Write},
    path::Path,
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, File},
};
use tracing::{debug, instrument, warn};

use super::{
    checksum::{
        CATEGORY_CHECKSUM,
        fetch_expected_checksum,
        map_ureq_error,
        verify_archive_checksum,
    },
    installer::DependencyBinaryInstallError,
};
use crate::artefact::download::HttpDownloader;

const DOWNLOAD_TIMEOUT_SECS: u64 = 30;

/// Maximum archive size accepted, so a runaway response cannot fill the disk.
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;

// Bounded `category` field on every boundary event, kept stable so operators can
// aggregate failures without unbounded cardinality. `checksum` is owned by
// `super::checksum` (imported above); the rest are local:
const CATEGORY_UTF8: &str = "utf8";
const CATEGORY_CAPABILITY: &str = "capability";
const CATEGORY_FETCH: &str = "fetch";
const CATEGORY_WRITE: &str = "write";

// Bounded `checksum_state` values the orchestrator reports (`parsed` is emitted
// by `super::checksum`).
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
/// production orchestration (agent construction and release-URL derivation)
/// while letting tests drive the full boundary sequence against a local server;
/// the public API exposes no URL override. The destination is validated and its
/// capability opened before the first HTTP request, so an invalid destination
/// fails without any network access.
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
    log_download_start();

    // Acquire the parent-directory capability up front so every archive read and
    // write flows through it (never ambient `std::fs`); validation happens here,
    // before any HTTP request.
    let destination_handle = open_download_destination(destination)?;
    destination_handle.download_archive(agent, archive_url)?;
    // Any failure after the archive is written removes it, so a retry never sees
    // a partial or unverified file.
    let expected_checksum =
        fetch_checksum_discarding_archive(agent, checksum_url, &destination_handle)?;
    verify_downloaded_archive_discarding_failure(
        archive_url,
        &destination_handle,
        &expected_checksum,
    )
}

/// Report that a dependency archive download is starting.
fn log_download_start() {
    debug!("starting dependency archive download");
}

/// Fetch the expected checksum, removing the written archive if it fails.
fn fetch_checksum_discarding_archive(
    agent: &ureq::Agent,
    checksum_url: &str,
    destination: &DownloadDestination,
) -> Result<String, DependencyBinaryInstallError> {
    fetch_expected_checksum(agent, checksum_url)
        .inspect_err(|_| destination.remove_partial_archive())
}

/// Re-open the written archive, verify it, and remove it on any failure.
///
/// The reopen is read-only. Both ways verification can fail — the archive
/// cannot be reopened, or its checksum does not match — reach
/// [`report_verification_discarding_failure`], so the cleanup that deletes the
/// archive lives at this command rather than in the read helper.
fn verify_downloaded_archive_discarding_failure(
    archive_url: &str,
    destination: &DownloadDestination,
    expected_checksum: &str,
) -> Result<(), DependencyBinaryInstallError> {
    // `verify_archive_checksum` consumes the reader, so the handle is closed
    // before the cleanup below runs.
    let verification = reopen_for_verification(destination).and_then(|archive| {
        verify_archive_checksum(archive, destination.path.as_std_path(), expected_checksum)
    });
    report_verification_discarding_failure(archive_url, destination, verification)
}

/// Report the verification outcome, removing the archive when it did not verify.
///
/// Reaching here is what makes the archive unsafe to keep: it either could not
/// be read back or failed its checksum, so a retry must never see it.
fn report_verification_discarding_failure(
    archive_url: &str,
    destination: &DownloadDestination,
    verification: Result<(), DependencyBinaryInstallError>,
) -> Result<(), DependencyBinaryInstallError> {
    if let Err(error) = verification {
        log_verification_failure(archive_url, destination, &error);
        // Remove the unverified archive so a retry never observes stale data.
        destination.remove_partial_archive();
        return Err(error);
    }
    log_checksum_verified();
    Ok(())
}

/// Report why verification failed, in exactly one event.
///
/// Only [`DependencyBinaryInstallError::Checksum`] means the archive was read
/// back and disagreed with its published digest. A reopen or read failure is a
/// capability problem, so reporting it as a checksum mismatch would label the
/// bytes as corrupt and hide the actual cause.
fn log_verification_failure(
    archive_url: &str,
    destination: &DownloadDestination,
    error: &DependencyBinaryInstallError,
) {
    if matches!(error, DependencyBinaryInstallError::Checksum { .. }) {
        log_checksum_mismatch(archive_url, error);
    } else {
        log_unreadable_archive(archive_url, destination, error);
    }
}

/// Report an archive whose digest differs from the published checksum.
fn log_checksum_mismatch(archive_url: &str, error: &DependencyBinaryInstallError) {
    warn!(
        category = CATEGORY_CHECKSUM,
        checksum_state = CHECKSUM_STATE_MISMATCH,
        url = %archive_url,
        error = %error,
        "archive checksum mismatch",
    );
}

/// Report an archive that could not be read back, so no checksum was computed.
fn log_unreadable_archive(
    archive_url: &str,
    destination: &DownloadDestination,
    error: &DependencyBinaryInstallError,
) {
    warn!(
        category = CATEGORY_CAPABILITY,
        archive_name = %destination.archive_name,
        url = %archive_url,
        error = %error,
        "archive could not be read back for verification",
    );
}

/// Report an archive that verified against its published checksum.
fn log_checksum_verified() {
    debug!(
        category = CATEGORY_CHECKSUM,
        checksum_state = CHECKSUM_STATE_VERIFIED,
        "archive checksum verified",
    );
}

/// Re-open the written archive for verification.
///
/// Read-only by design: the command that owns the discarding removes the
/// archive and reports why, so a warning here would be a second event for one
/// failure.
fn reopen_for_verification(
    destination: &DownloadDestination,
) -> Result<cap_std::fs_utf8::File, DependencyBinaryInstallError> {
    destination.open_archive().map_err(Into::into)
}

/// A validated archive destination — UTF-8 path, capability-scoped parent
/// directory, and archive file name — bundled so archive operations do not
/// thread repeated `&str`/`&Dir` parameters.
struct DownloadDestination {
    path: Utf8PathBuf,
    dir: Dir,
    archive_name: String,
}

impl DownloadDestination {
    /// Fetch the archive at `url` and write it into the capability directory,
    /// removing a partial file if the write fails. Fetch failures map through
    /// [`map_ureq_error`]; capability-create and write failures propagate.
    fn download_archive(
        &self,
        agent: &ureq::Agent,
        url: &str,
    ) -> Result<(), DependencyBinaryInstallError> {
        let response = agent
            .get(url)
            .call()
            .map_err(|error| map_ureq_error(url, &error))
            .inspect_err(|error| {
                warn!(category = CATEGORY_FETCH, url = %url, error = %error, "archive fetch failed");
            })?;
        let mut file = self.dir.create(&self.archive_name).inspect_err(|error| {
            warn!(
                category = CATEGORY_CAPABILITY,
                archive_name = %self.archive_name,
                error = %error,
                "failed to create archive file",
            );
        })?;
        let mut body = response.into_body();
        let reader = body.as_reader();
        let copy_result = copy_capped(reader, &mut file, MAX_ARCHIVE_BYTES, url);
        drop(file);
        if let Err(error) = copy_result {
            warn!(category = CATEGORY_WRITE, url = %url, error = %error, "failed to write archive to disk");
            // Remove the partially written archive so a retry starts clean.
            self.remove_partial_archive();
            return Err(error);
        }
        Ok(())
    }

    /// Reopen the written archive through the capability for verification.
    fn open_archive(&self) -> io::Result<File> { self.dir.open(&self.archive_name) }

    /// Remove a partial or unverified archive; a cleanup failure is only logged.
    fn remove_partial_archive(&self) {
        if let Err(error) = self.dir.remove_file(&self.archive_name) {
            warn!(
                category = CATEGORY_CAPABILITY,
                archive_name = %self.archive_name,
                error = %error,
                "failed to remove archive after a download failure",
            );
        }
    }
}

/// Validate `destination` as UTF-8 and open its parent directory as a
/// capability, returning an owned [`DownloadDestination`] so the caller has no
/// borrowed lifetimes.
///
/// # Errors
///
/// Returns [`DependencyBinaryInstallError::Io`] with
/// [`io::ErrorKind::InvalidInput`] when `destination` is not valid UTF-8, and
/// propagates capability-open failures from [`open_destination_dir`].
fn open_download_destination(
    destination: &Path,
) -> Result<DownloadDestination, DependencyBinaryInstallError> {
    let path = Utf8Path::from_path(destination).ok_or_else(|| {
        warn!(
            category = CATEGORY_UTF8,
            "destination archive path is not valid UTF-8",
        );
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination archive path is not valid UTF-8",
        )
    })?;
    let (dir, archive_name) = open_destination_dir(path).inspect_err(|error| {
        warn!(
            category = CATEGORY_CAPABILITY,
            destination = %path,
            error = %error,
            "failed to open destination directory",
        );
    })?;
    Ok(DownloadDestination {
        path: path.to_owned(),
        dir,
        archive_name: archive_name.to_owned(),
    })
}

/// Copy at most `max_bytes` from `reader` into `writer`; an over-cap response
/// becomes a `Download` error rather than being written in full.
fn copy_capped(
    mut reader: impl Read,
    writer: &mut impl Write,
    max_bytes: u64,
    url: &str,
) -> Result<(), DependencyBinaryInstallError> {
    let copied = io::copy(&mut reader.by_ref().take(max_bytes), writer)?;
    // Probe via `io::copy`, not a bare `read`, so `Interrupted` is retried.
    if copied == max_bytes && io::copy(&mut reader.by_ref().take(1), &mut io::sink())? > 0 {
        return Err(DependencyBinaryInstallError::Download {
            url: url.to_owned(),
            reason: format!("archive exceeds the maximum of {max_bytes} bytes"),
        });
    }
    Ok(())
}

/// Open the parent directory of `destination` as a capability, returning it
/// alongside the archive's file name. `cap_std` grants no ambient authority, so
/// the parent is opened explicitly and all subsequent archive I/O is scoped to
/// the returned handle rather than ambient `std::fs`.
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

#[cfg(test)]
#[path = "downloader_tests.rs"]
mod tests;
