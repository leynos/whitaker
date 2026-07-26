//! End-to-end boundary tests for the dependency-archive download workflow.
//!
//! These drive [`super::downloader::download_from_urls`] against a loopback-only
//! HTTP server so the full validate → fetch → write → checksum → verify sequence
//! runs without the network. The server helper lives here (not in `downloader.rs`)
//! to keep that module within its size budget; it is test support for these cases.

use super::downloader::{
    DependencyArchiveDownloader, RepositoryArchiveDownloader, download_from_urls,
};
use super::installer::DependencyBinaryInstallError;
use crate::hex::to_lower_hex;
use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tempfile::TempDir;

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

/// A loopback-only HTTP/1.1 server for the download workflow. It answers a fixed
/// route table, records requested paths, and shuts down cleanly on drop.
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

/// Accept connections until `stop` is set, serving each through the route table.
/// Non-blocking polling keeps the loop responsive to shutdown when idle.
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
    // Restore blocking mode and bound reads/writes on the accepted connection
    // (the listener is non-blocking only so the accept loop can poll for
    // shutdown). `try_clone` shares the socket, so `peer` inherits these.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
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
        concat!(
            "HTTP/1.1 {}\r\n",
            "Content-Length: {}\r\n",
            "Content-Type: application/octet-stream\r\n",
            "Connection: close\r\n\r\n",
        ),
        status_line,
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

    // The unverified archive must not survive a checksum mismatch, so a retry
    // never reads stale data from the destination.
    let temp_dir = Utf8Path::from_path(temp.path()).expect("temp path is UTF-8");
    let dir =
        Dir::open_ambient_dir(temp_dir, ambient_authority()).expect("open temp dir capability");
    assert!(
        !dir.exists("archive.tgz"),
        "archive must be removed from the destination after a checksum mismatch",
    );
}

#[test]
fn download_from_urls_accepts_an_uppercase_checksum_sidecar() {
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
    let server = LocalServer::start(routes);

    let temp = TempDir::new().expect("create temp dir");
    let destination = temp.path().join("archive.tgz");

    download_from_urls(
        &test_agent(),
        &server.url("/archive.tgz"),
        &server.url("/archive.tgz.sha256"),
        &destination,
    )
    .expect("an upper-case checksum sidecar must still verify");
}

#[test]
fn download_from_urls_reports_an_empty_checksum_sidecar_with_its_url() {
    // A blank sidecar has no token; the workflow maps the pure parser's `None`
    // to a URL-bearing `Download` error identifying the checksum endpoint.
    let archive_bytes = b"whitaker dependency archive payload".to_vec();
    let mut routes = HashMap::new();
    routes.insert("/archive.tgz".to_owned(), CannedResponse::ok(archive_bytes));
    routes.insert(
        "/archive.tgz.sha256".to_owned(),
        CannedResponse::ok(b"   \n".to_vec()),
    );
    let server = LocalServer::start(routes);

    let temp = TempDir::new().expect("create temp dir");
    let destination = temp.path().join("archive.tgz");
    let checksum_url = server.url("/archive.tgz.sha256");

    let error = download_from_urls(
        &test_agent(),
        &server.url("/archive.tgz"),
        &checksum_url,
        &destination,
    )
    .expect_err("a blank checksum sidecar must fail");

    match error {
        DependencyBinaryInstallError::Download { url, reason } => {
            assert_eq!(url, checksum_url);
            assert_eq!(reason, "empty or invalid checksum file");
        }
        other => panic!("expected a Download error, got {other:?}"),
    }
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
