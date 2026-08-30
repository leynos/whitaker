//! Tests for downloader error mapping and archive checksum verification.

//! Tests for downloader error mapping and archive checksum verification.

use std::io::Write;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::*;
use crate::hex::to_lower_hex;

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
        if self.dir.remove_file(&self.name).is_err() {
            // A correct run never writes the probe into this directory, so
            // a missing file is the expected case.
        }
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
    let destination_dir =
        Dir::open_ambient_dir(temp_dir, ambient_authority()).expect("open destination capability");

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
