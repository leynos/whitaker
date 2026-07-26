//! Download support for repository-hosted dependency-binary archives.

use crate::artefact::download::HttpDownloader;

use super::installer::DependencyBinaryInstallError;
use crate::hex::to_lower_hex;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use sha2::{Digest, Sha256};
use std::io;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument, warn};

const DOWNLOAD_TIMEOUT_SECS: u64 = 30;

// Bounded `category` field emitted on every boundary event below. Keeping the
// set stable lets operators aggregate download failures by category without
// unbounded label cardinality.
//
// - `utf8`: the destination path is not valid UTF-8.
// - `capability`: opening, creating, or reopening through the `cap_std`
//   directory capability failed.
// - `fetch`: a network request (archive or checksum) failed.
// - `write`: streaming the fetched archive to local disk failed.
// - `checksum`: fetching, reading, parsing, or verifying the checksum failed.
const CATEGORY_UTF8: &str = "utf8";
const CATEGORY_CAPABILITY: &str = "capability";
const CATEGORY_FETCH: &str = "fetch";
const CATEGORY_WRITE: &str = "write";
const CATEGORY_CHECKSUM: &str = "checksum";

// Bounded `checksum_state` field values marking the checksum-processing stage.
const CHECKSUM_STATE_PARSED: &str = "parsed";
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
        let archive_url = asset_url(filename);
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
fn download_from_urls(
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
    // it; the checksum helper stays pure over the reader.
    let archive = dir.open(&archive_name).inspect_err(|error| {
        warn!(
            category = CATEGORY_CAPABILITY,
            archive_name = %archive_name,
            error = %error,
            "failed to reopen archive for verification",
        );
    })?;
    verify_archive_checksum(archive, destination.as_std_path(), &expected_checksum)
        .inspect(|_| {
            debug!(
                category = CATEGORY_CHECKSUM,
                checksum_state = CHECKSUM_STATE_VERIFIED,
                "archive checksum verified",
            );
        })
        .inspect_err(|error| {
            warn!(
                category = CATEGORY_CHECKSUM,
                checksum_state = CHECKSUM_STATE_MISMATCH,
                url = %archive_url,
                error = %error,
                "archive checksum verification failed",
            );
        })
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

/// Fetch the `.sha256` sidecar at `checksum_url` and return the expected digest.
///
/// The token is lowercased so an upper-case sidecar digest still compares equal
/// to [`compute_sha256`]'s lower-case output.
///
/// # Errors
///
/// Returns [`DependencyBinaryInstallError::Download`] on fetch, body-read, or
/// malformed/empty checksum failures.
fn fetch_expected_checksum(
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
    // Normalize to lower case so an upper-case sidecar digest still matches the
    // lower-case digest produced by `compute_sha256`.
    let expected = parse_checksum_token(&checksum_body, checksum_url)?.to_ascii_lowercase();
    debug!(
        category = CATEGORY_CHECKSUM,
        checksum_state = CHECKSUM_STATE_PARSED,
        url = %checksum_url,
        "parsed expected checksum",
    );
    Ok(expected)
}

/// Extract the digest token (first whitespace-delimited field of the first
/// line) from a checksum-file body.
///
/// # Errors
///
/// Returns [`DependencyBinaryInstallError::Download`] with an
/// `empty or invalid checksum file` reason when no token is present.
fn parse_checksum_token<'body>(
    body: &'body str,
    checksum_url: &str,
) -> Result<&'body str, DependencyBinaryInstallError> {
    body.lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(|| {
            warn!(
                category = CATEGORY_CHECKSUM,
                url = %checksum_url,
                "empty or invalid checksum file",
            );
            DependencyBinaryInstallError::Download {
                url: checksum_url.to_owned(),
                reason: "empty or invalid checksum file".to_string(),
            }
        })
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
fn verify_archive_checksum(
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
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;
    use tempfile::{NamedTempFile, TempDir};

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
    fn parse_checksum_token_returns_the_first_field_of_the_first_line() {
        let body = "abc123def456  whitaker-tool.tgz\nignored second line\n";
        assert_eq!(
            parse_checksum_token(body, "https://example.test/archive.tgz.sha256")
                .expect("token present"),
            "abc123def456",
        );
    }

    #[rstest]
    #[case("")]
    #[case("   \n")]
    #[case("\n\n")]
    fn parse_checksum_token_rejects_an_empty_or_blank_body(#[case] body: &str) {
        let error = parse_checksum_token(body, "https://example.test/archive.tgz.sha256")
            .expect_err("blank checksum body must fail");
        match error {
            DependencyBinaryInstallError::Download { url, reason } => {
                assert_eq!(url, "https://example.test/archive.tgz.sha256");
                assert_eq!(reason, "empty or invalid checksum file");
            }
            other => panic!("expected a Download error, got {other:?}"),
        }
    }

    /// Reopen `file` as a fresh read handle for streaming into the hasher.
    fn read_handle(file: &NamedTempFile) -> impl Read {
        file.reopen().expect("reopen temp file")
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

    #[cfg(unix)]
    #[test]
    fn download_rejects_a_non_utf8_destination_before_any_network_access() {
        use std::os::unix::ffi::OsStringExt as _;

        // 0x80 is a lone UTF-8 continuation byte, so this path is never valid
        // UTF-8. The download must reject it during path validation, which
        // happens before any HTTP call, so the test needs no network.
        let invalid = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', 0x80, b'.', b't', b'g', b'z',
        ]));

        let downloader = RepositoryArchiveDownloader;
        let error = downloader
            .download("whitaker-dependency", &invalid)
            .expect_err("non-UTF-8 destination must be rejected");

        match error {
            DependencyBinaryInstallError::Io(source) => {
                assert_eq!(source.kind(), io::ErrorKind::InvalidInput);
                assert!(
                    source
                        .to_string()
                        .contains("destination archive path is not valid UTF-8"),
                    "error should identify the non-UTF-8 destination, got: {source}",
                );
            }
            other => panic!("expected an Io error, got {other:?}"),
        }
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

    /// One canned HTTP/1.1 response body served for a matched path.
    struct CannedResponse {
        status_line: &'static str,
        body: Vec<u8>,
    }

    impl CannedResponse {
        fn ok(body: Vec<u8>) -> Self {
            Self {
                status_line: "200 OK",
                body,
            }
        }
    }

    /// A loopback-only HTTP/1.1 server for exercising the download workflow
    /// without touching the network. It answers a fixed route table, records the
    /// requested paths, and shuts down cleanly on drop even if no request
    /// arrives.
    struct LocalServer {
        base_url: String,
        requested: Arc<Mutex<Vec<String>>>,
        stop: Arc<AtomicBool>,
        handle: Option<JoinHandle<()>>,
    }

    impl LocalServer {
        fn start(routes: HashMap<String, CannedResponse>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
            let port = listener.local_addr().expect("resolve local addr").port();
            listener
                .set_nonblocking(true)
                .expect("set listener non-blocking");
            let requested = Arc::new(Mutex::new(Vec::new()));
            let stop = Arc::new(AtomicBool::new(false));
            let handle = {
                let requested = Arc::clone(&requested);
                let stop = Arc::clone(&stop);
                thread::spawn(move || run_server(&listener, &routes, &requested, &stop))
            };
            Self {
                base_url: format!("http://127.0.0.1:{port}"),
                requested,
                stop,
                handle: Some(handle),
            }
        }

        fn url(&self, path: &str) -> String {
            format!("{}{path}", self.base_url)
        }

        fn requested_paths(&self) -> Vec<String> {
            self.requested.lock().expect("lock requested paths").clone()
        }
    }

    impl Drop for LocalServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    /// Accept connections until `stop` is set, serving each through the route
    /// table. Non-blocking polling keeps the loop responsive to shutdown even
    /// when no request ever arrives.
    fn run_server(
        listener: &TcpListener,
        routes: &HashMap<String, CannedResponse>,
        requested: &Arc<Mutex<Vec<String>>>,
        stop: &AtomicBool,
    ) {
        while !stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => serve_connection(stream, routes, requested),
                Err(ref error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    }

    /// Read one request, record its path, and write the matching canned response
    /// (or a 404). `Connection: close` lets the client frame the response end.
    fn serve_connection(
        mut stream: TcpStream,
        routes: &HashMap<String, CannedResponse>,
        requested: &Arc<Mutex<Vec<String>>>,
    ) {
        let Ok(peer) = stream.try_clone() else {
            return;
        };
        let mut reader = BufReader::new(peer);
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
            return;
        }
        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_owned();
        // Drain the remaining request headers up to the blank line.
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) if line == "\r\n" || line == "\n" => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
        requested
            .lock()
            .expect("lock requested paths")
            .push(path.clone());

        let (status_line, body): (&str, &[u8]) = match routes.get(&path) {
            Some(response) => (response.status_line, &response.body),
            None => ("404 Not Found", b"not found"),
        };
        let header = format!(
            "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n\
             Content-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
            body.len(),
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body);
        let _ = stream.flush();
    }

    /// A short-timeout agent for driving the local server.
    fn test_agent() -> ureq::Agent {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build();
        ureq::Agent::new_with_config(config)
    }

    #[test]
    fn download_from_urls_writes_the_archive_and_requests_both_endpoints() {
        let archive_bytes = b"whitaker dependency archive payload".to_vec();
        let checksum = to_lower_hex(&Sha256::digest(&archive_bytes));
        let mut routes = HashMap::new();
        routes.insert(
            "/archive.tgz".to_owned(),
            CannedResponse::ok(archive_bytes.clone()),
        );
        routes.insert(
            "/archive.tgz.sha256".to_owned(),
            CannedResponse::ok(format!("{checksum}  archive.tgz\n").into_bytes()),
        );
        let server = LocalServer::start(routes);

        let temp = TempDir::new().expect("create temp dir");
        let destination = temp.path().join("archive.tgz");

        download_from_urls(
            &test_agent(),
            &server.url("/archive.tgz"),
            &server.url("/archive.tgz.sha256"),
            &destination,
        )
        .expect("download succeeds");

        // Read the archive back through a capability to confirm the exact bytes
        // landed at the requested destination.
        let temp_dir = Utf8Path::from_path(temp.path()).expect("temp path is UTF-8");
        let dir =
            Dir::open_ambient_dir(temp_dir, ambient_authority()).expect("open temp dir capability");
        let mut written = Vec::new();
        dir.open("archive.tgz")
            .expect("open written archive")
            .read_to_end(&mut written)
            .expect("read written archive");
        assert_eq!(written, archive_bytes);

        let requested = server.requested_paths();
        assert!(requested.contains(&"/archive.tgz".to_owned()));
        assert!(requested.contains(&"/archive.tgz.sha256".to_owned()));
    }

    #[test]
    fn download_from_urls_reports_a_checksum_mismatch() {
        let archive_bytes = b"whitaker dependency archive payload".to_vec();
        // Syntactically valid but incorrect (all-zero) digest.
        let wrong_checksum = "0".repeat(64);
        let mut routes = HashMap::new();
        routes.insert("/archive.tgz".to_owned(), CannedResponse::ok(archive_bytes));
        routes.insert(
            "/archive.tgz.sha256".to_owned(),
            CannedResponse::ok(format!("{wrong_checksum}  archive.tgz\n").into_bytes()),
        );
        let server = LocalServer::start(routes);

        let temp = TempDir::new().expect("create temp dir");
        let destination = temp.path().join("archive.tgz");

        let error = download_from_urls(
            &test_agent(),
            &server.url("/archive.tgz"),
            &server.url("/archive.tgz.sha256"),
            &destination,
        )
        .expect_err("mismatched checksum must fail");

        match error {
            DependencyBinaryInstallError::Checksum {
                archive,
                expected,
                actual,
            } => {
                assert_eq!(archive, destination);
                assert_eq!(expected, wrong_checksum);
                assert_ne!(actual, expected);
                assert_eq!(actual.len(), 64);
            }
            other => panic!("expected a Checksum error, got {other:?}"),
        }

        let requested = server.requested_paths();
        assert!(requested.contains(&"/archive.tgz".to_owned()));
        assert!(requested.contains(&"/archive.tgz.sha256".to_owned()));
    }

    #[cfg(unix)]
    #[test]
    fn download_from_urls_rejects_a_non_utf8_destination_before_any_request() {
        use std::os::unix::ffi::OsStringExt as _;

        // No route is registered; the server exists only to prove it is never
        // contacted for an invalid destination.
        let server = LocalServer::start(HashMap::new());

        // 0x80 is a lone UTF-8 continuation byte, so this path is never valid
        // UTF-8 and must be rejected during validation, before any HTTP request.
        let invalid = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', 0x80, b'.', b't', b'g', b'z',
        ]));

        let error = download_from_urls(
            &test_agent(),
            &server.url("/archive.tgz"),
            &server.url("/archive.tgz.sha256"),
            &invalid,
        )
        .expect_err("non-UTF-8 destination must be rejected");

        match error {
            DependencyBinaryInstallError::Io(source) => {
                assert_eq!(source.kind(), io::ErrorKind::InvalidInput);
            }
            other => panic!("expected an Io error, got {other:?}"),
        }
        assert!(
            server.requested_paths().is_empty(),
            "no HTTP request must be made for an invalid destination",
        );
    }
}
