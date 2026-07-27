//! End-to-end boundary tests for the dependency-archive download workflow.
//!
//! These drive [`super::downloader::download_from_urls`] against a loopback-only
//! HTTP server so the full validate → fetch → write → checksum → verify sequence
//! runs without the network. The server helper lives here (not in `downloader.rs`)
//! to keep that module within its size budget; it is test support for these cases.

use super::downloader::download_from_urls;
// Only the non-UTF-8 production-path test (gated `#[cfg(unix)]`) drives the
// trait and concrete downloader; keep these imports on the same gate so other
// platforms do not see them as unused.
#[cfg(unix)]
use super::downloader::{DependencyArchiveDownloader, RepositoryArchiveDownloader};
use super::http_test_server::{CannedResponse, LocalServer};
use super::installer::DependencyBinaryInstallError;
use crate::hex::to_lower_hex;
use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest::{fixture, rstest};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
// `io` is only referenced by the `#[cfg(unix)]` non-UTF-8 tests (`io::ErrorKind`);
// `Read` is used across platforms for `read_to_end`.
#[cfg(unix)]
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

/// A short-timeout agent fixture for driving the local server.
#[fixture]
fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build();
    ureq::Agent::new_with_config(config)
}

/// A running local server plus a temporary destination for `archive.tgz`. The
/// temp dir is retained so the destination outlives the test body.
struct DownloadHarness {
    server: LocalServer,
    _temp: TempDir,
    destination: PathBuf,
}

impl DownloadHarness {
    fn archive_url(&self) -> String {
        self.server.url("/archive.tgz")
    }

    fn checksum_url(&self) -> String {
        self.server.url("/archive.tgz.sha256")
    }

    fn requested_paths(&self) -> Vec<String> {
        self.server.requested_paths()
    }

    /// Open the destination's parent directory as a capability, for asserting on
    /// the written archive.
    fn destination_dir(&self) -> Dir {
        let parent = Utf8Path::from_path(
            self.destination
                .parent()
                .expect("destination has a parent directory"),
        )
        .expect("temp path is UTF-8");
        Dir::open_ambient_dir(parent, ambient_authority()).expect("open temp dir capability")
    }
}

/// Start a local server for `routes` and a temp destination for the archive.
///
/// This is a plain constructor rather than an `#[fixture]` because the route
/// table varies per test and depends on test-local data, which rstest's
/// fixture injection cannot supply.
fn download_harness(routes: HashMap<String, CannedResponse>) -> DownloadHarness {
    let server = LocalServer::start(routes);
    let temp = TempDir::new().expect("create temp dir");
    let destination = temp.path().join("archive.tgz");
    DownloadHarness {
        server,
        _temp: temp,
        destination,
    }
}

#[rstest]
fn download_from_urls_writes_the_archive_and_requests_both_endpoints(agent: ureq::Agent) {
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
    let harness = download_harness(routes);

    download_from_urls(
        &agent,
        &harness.archive_url(),
        &harness.checksum_url(),
        &harness.destination,
    )
    .expect("download succeeds");

    // Read the archive back through a capability to confirm the exact bytes
    // landed at the requested destination.
    let mut written = Vec::new();
    harness
        .destination_dir()
        .open("archive.tgz")
        .expect("open written archive")
        .read_to_end(&mut written)
        .expect("read written archive");
    assert_eq!(written, archive_bytes);

    let requested = harness.requested_paths();
    assert!(requested.contains(&"/archive.tgz".to_owned()));
    assert!(requested.contains(&"/archive.tgz.sha256".to_owned()));
}

#[rstest]
fn download_from_urls_reports_a_checksum_mismatch(agent: ureq::Agent) {
    let archive_bytes = b"whitaker dependency archive payload".to_vec();
    // Syntactically valid but incorrect (all-zero) digest.
    let wrong_checksum = "0".repeat(64);
    let mut routes = HashMap::new();
    routes.insert("/archive.tgz".to_owned(), CannedResponse::ok(archive_bytes));
    routes.insert(
        "/archive.tgz.sha256".to_owned(),
        CannedResponse::ok(format!("{wrong_checksum}  archive.tgz\n").into_bytes()),
    );
    let harness = download_harness(routes);

    let error = download_from_urls(
        &agent,
        &harness.archive_url(),
        &harness.checksum_url(),
        &harness.destination,
    )
    .expect_err("mismatched checksum must fail");

    match error {
        DependencyBinaryInstallError::Checksum {
            archive,
            expected,
            actual,
        } => {
            assert_eq!(archive, harness.destination);
            assert_eq!(expected, wrong_checksum);
            assert_ne!(actual, expected);
            assert_eq!(actual.len(), 64);
        }
        other => panic!("expected a Checksum error, got {other:?}"),
    }

    let requested = harness.requested_paths();
    assert!(requested.contains(&"/archive.tgz".to_owned()));
    assert!(requested.contains(&"/archive.tgz.sha256".to_owned()));

    // The unverified archive must not survive a checksum mismatch, so a retry
    // never reads stale data from the destination.
    assert!(
        !harness.destination_dir().exists("archive.tgz"),
        "archive must be removed from the destination after a checksum mismatch",
    );
}

#[rstest]
fn download_from_urls_accepts_an_uppercase_checksum_sidecar(agent: ureq::Agent) {
    // Serve the correct digest in UPPER CASE: the download must still verify,
    // proving `fetch_expected_checksum` normalizes the sidecar token to lower
    // case before comparison.
    let archive_bytes = b"whitaker dependency archive payload".to_vec();
    let checksum = to_lower_hex(&Sha256::digest(&archive_bytes)).to_ascii_uppercase();
    let mut routes = HashMap::new();
    routes.insert("/archive.tgz".to_owned(), CannedResponse::ok(archive_bytes));
    routes.insert(
        "/archive.tgz.sha256".to_owned(),
        CannedResponse::ok(format!("{checksum}  archive.tgz\n").into_bytes()),
    );
    let harness = download_harness(routes);

    download_from_urls(
        &agent,
        &harness.archive_url(),
        &harness.checksum_url(),
        &harness.destination,
    )
    .expect("an upper-case checksum sidecar must still verify");
}

#[rstest]
fn download_from_urls_reports_an_empty_checksum_sidecar_with_its_url(agent: ureq::Agent) {
    // A blank sidecar has no token; the workflow maps the pure parser's `None`
    // to a URL-bearing `Download` error identifying the checksum endpoint.
    let archive_bytes = b"whitaker dependency archive payload".to_vec();
    let mut routes = HashMap::new();
    routes.insert("/archive.tgz".to_owned(), CannedResponse::ok(archive_bytes));
    routes.insert(
        "/archive.tgz.sha256".to_owned(),
        CannedResponse::ok(b"   \n".to_vec()),
    );
    let harness = download_harness(routes);
    let checksum_url = harness.checksum_url();

    let error = download_from_urls(
        &agent,
        &harness.archive_url(),
        &checksum_url,
        &harness.destination,
    )
    .expect_err("a blank checksum sidecar must fail");

    match error {
        DependencyBinaryInstallError::Download { url, reason } => {
            assert_eq!(url, checksum_url);
            assert_eq!(reason, "empty or invalid checksum file");
        }
        other => panic!("expected a Download error, got {other:?}"),
    }

    // The archive was written before the checksum failed, so it must be removed.
    assert!(
        !harness.destination_dir().exists("archive.tgz"),
        "archive must be removed after a checksum retrieval failure",
    );
}

#[cfg(unix)]
#[test]
fn download_from_urls_rejects_a_non_utf8_destination_before_any_request() {
    use std::os::unix::ffi::OsStringExt as _;

    // No route is registered; the server exists only to prove it is never
    // contacted for an invalid destination.
    let server = LocalServer::start(HashMap::new());

    // 0x80 is a lone UTF-8 continuation byte, so this path is never valid UTF-8
    // and must be rejected during validation, before any HTTP request.
    let invalid = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![
        b'/', b't', b'm', b'p', b'/', 0x80, b'.', b't', b'g', b'z',
    ]));

    let error = download_from_urls(
        &agent(),
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

#[cfg(unix)]
#[test]
fn download_rejects_a_non_utf8_destination_before_any_network_access() {
    use std::os::unix::ffi::OsStringExt as _;

    // 0x80 is a lone UTF-8 continuation byte, so this path is never valid UTF-8.
    // The production `download` must reject it during path validation, which
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
