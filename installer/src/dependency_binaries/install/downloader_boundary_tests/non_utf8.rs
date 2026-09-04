//! Unix-only non-UTF-8 destination boundary tests.

use std::io;

use rstest::fixture;

use super::super::downloader::{DependencyArchiveDownloader, RepositoryArchiveDownloader};
use super::*;

#[fixture]
fn non_utf8_destination() -> std::path::PathBuf {
    use std::os::unix::ffi::OsStringExt as _;

    // 0x80 is a lone UTF-8 continuation byte, so this path is never valid UTF-8.
    std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![
        b'/', b't', b'm', b'p', b'/', 0x80, b'.', b't', b'g', b'z',
    ]))
}

#[rstest::rstest]
fn download_from_urls_rejects_a_non_utf8_destination_before_any_request(
    non_utf8_destination: std::path::PathBuf,
) {
    // No route is registered; the server exists only to prove it is never
    // contacted for an invalid destination.
    let server = LocalServer::start(HashMap::new());

    let error = download_from_urls(
        &agent(),
        &server.url("/archive.tgz"),
        &server.url("/archive.tgz.sha256"),
        &non_utf8_destination,
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

#[rstest::rstest]
fn download_rejects_a_non_utf8_destination_before_any_network_access(
    non_utf8_destination: std::path::PathBuf,
) {
    // The production `download` must reject it during path validation, which
    // happens before any HTTP call, so the test needs no network.
    let downloader = RepositoryArchiveDownloader;
    let error = downloader
        .download("whitaker-dependency", &non_utf8_destination)
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
