//! Download support for repository-hosted dependency-binary archives.

use crate::artefact::download::HttpDownloader;

use super::checksum::{fetch_expected_checksum, map_ureq_error, verify_archive_checksum};
use super::installer::DependencyBinaryInstallError;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use std::io::{self, Read, Write};
use std::path::Path;
use tracing::{debug, instrument, warn};

const DOWNLOAD_TIMEOUT_SECS: u64 = 30;

/// Maximum archive size accepted, so a runaway response cannot fill the disk.
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;

// Bounded `category` field on every boundary event, kept stable so operators can
// aggregate failures without unbounded cardinality: `utf8`, `capability` (cap_std
// dir op), `fetch` (network), `write` (archive-to-disk), `checksum` (shared).
const CATEGORY_UTF8: &str = "utf8";
const CATEGORY_CAPABILITY: &str = "capability";
const CATEGORY_FETCH: &str = "fetch";
const CATEGORY_WRITE: &str = "write";
// Shared with `super::checksum`, which emits this category on its own events.
pub(super) const CATEGORY_CHECKSUM: &str = "checksum";

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
    debug!("starting dependency archive download");

    // Acquire the parent-directory capability up front so every archive read and
    // write flows through it (never ambient `std::fs`); validation happens here,
    // before any HTTP request.
    let (destination, dir, archive_name) = open_download_destination(destination)?;
    download_archive(agent, archive_url, &dir, &archive_name)?;
    // Any failure after the archive is written removes it, so a retry never sees
    // a partial or unverified file.
    let expected_checksum = fetch_expected_checksum(agent, checksum_url)
        .inspect_err(|_| remove_partial_archive(&dir, &archive_name))?;

    // Re-open the written archive and verify it; `verify_archive_checksum`
    // consumes the reader, closing the handle before any cleanup below.
    let archive = dir.open(&archive_name).inspect_err(|error| {
        warn!(
            category = CATEGORY_CAPABILITY,
            archive_name = %archive_name,
            error = %error,
            "failed to reopen archive for verification",
        );
        remove_partial_archive(&dir, &archive_name);
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
            // Remove the unverified archive so a retry never observes stale data.
            remove_partial_archive(&dir, &archive_name);
            Err(error)
        }
    }
}

/// Remove a partial or unverified archive; a cleanup failure is only logged.
fn remove_partial_archive(dir: &Dir, archive_name: &str) {
    if let Err(error) = dir.remove_file(archive_name) {
        warn!(
            category = CATEGORY_CAPABILITY,
            archive_name = %archive_name,
            error = %error,
            "failed to remove archive after a download failure",
        );
    }
}

/// Validate `destination` as UTF-8 and open its parent directory as a
/// capability, returning owned values so the caller has no borrowed lifetimes.
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
/// The response body is streamed only into the handle returned by `dir.create`,
/// which is dropped before returning.
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
    let reader = body.as_reader();
    let copy_result = copy_capped(reader, &mut file, MAX_ARCHIVE_BYTES, url);
    drop(file);
    if let Err(error) = copy_result {
        warn!(
            category = CATEGORY_WRITE,
            url = %url,
            error = %error,
            "failed to write archive to disk",
        );
        // Remove the partially written archive so a retry starts clean.
        remove_partial_archive(dir, archive_name);
        return Err(error);
    }
    Ok(())
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
    if copied == max_bytes && reader.read(&mut [0u8; 1])? > 0 {
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
mod tests {
    //! Tests for downloader error mapping and archive checksum verification.

    use super::*;
    use crate::hex::to_lower_hex;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use tempfile::TempDir;

    // The under-cap success path is covered end to end by the local-server
    // boundary tests; this exercises the over-cap rejection they cannot.
    #[test]
    fn copy_capped_rejects_a_body_exceeding_the_limit() {
        let url = "https://example.test/a.tgz";
        let error = copy_capped(&[0u8; 100][..], &mut Vec::new(), 8, url)
            .expect_err("an over-cap body must be rejected");
        assert!(
            matches!(&error, DependencyBinaryInstallError::Download { url: u, reason }
                if u == url && reason.contains("exceeds the maximum")),
            "unexpected error: {error:?}",
        );
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
