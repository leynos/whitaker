//! Release provisioning downloads through the ports, not the filesystem.
//!
//! The store is scripted, so these tests show that the release path asks it
//! for the bundle digest rather than hashing a real file behind its back.

use std::{path::Path, sync::Mutex};

use camino::Utf8Path;

use super::{
    super::{
        error::ClippySnapshotError,
        ports::SnapshotSupport,
        provision::{BundleSource, provision_support},
    },
    support::{Ports, TIP, manifest},
};
use crate::dependency_binaries::{DependencyArchiveDownloader, DependencyBinaryInstallError};

/// A downloader that records requests and writes nothing.
///
/// Writing nothing is the point: the scripted store supplies the digest, so a
/// path that still hashed the file would fail rather than pass by accident.
struct RecordingDownloader {
    requests: Mutex<Vec<String>>,
}

impl RecordingDownloader {
    /// Script a downloader that has served no request yet.
    fn new() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Return the asset names requested so far, in order.
    fn requests(&self) -> Vec<String> { self.requests.lock().expect("lock requests").clone() }
}

impl DependencyArchiveDownloader for RecordingDownloader {
    fn download(
        &self,
        filename: &str,
        _destination: &Path,
    ) -> Result<(), DependencyBinaryInstallError> {
        self.requests
            .lock()
            .expect("lock requests")
            .push(filename.to_owned());
        Ok(())
    }
}

#[test]
fn release_provisioning_reads_the_digest_through_the_store() {
    let root = Utf8Path::new("/root");
    let ports = Ports::accepting();
    let downloader = RecordingDownloader::new();
    let support = SnapshotSupport {
        downloader: &downloader,
        ..ports.support()
    };

    let generation = provision_support(&BundleSource::Release, &manifest(), root, &support)
        .expect("the scripted release installs");

    assert_eq!(generation, root.join(TIP));
    let asset = manifest().bundle_asset_name();
    assert_eq!(
        downloader.requests(),
        vec![asset.clone()],
        "the release asset must be requested once"
    );
    let asset_path = root.join(".download-fake").join(&asset);
    assert!(
        ports
            .store
            .calls()
            .contains(&format!("digest {asset_path}")),
        "the bundle digest must be read through the store: {:?}",
        ports.store.calls()
    );
    assert!(
        ports.store.has_call_starting_with("publish "),
        "the downloaded bundle must be installed: {:?}",
        ports.store.calls()
    );
}

#[test]
fn a_store_digest_failure_stops_provisioning_before_install() {
    let root = Utf8Path::new("/root");
    let mut ports = Ports::accepting();
    ports.store.digest = Err(ClippySnapshotError::Io {
        path: "snapshot.bundle".to_owned(),
        reason: "permission denied".to_owned(),
    });
    let downloader = RecordingDownloader::new();
    let support = SnapshotSupport {
        downloader: &downloader,
        ..ports.support()
    };

    let error = provision_support(&BundleSource::Release, &manifest(), root, &support)
        .expect_err("an unreadable digest is an error");

    assert!(
        matches!(&error, ClippySnapshotError::Io { reason, .. } if reason == "permission denied"),
        "{error}"
    );
    assert!(
        !ports.store.has_call_starting_with("lock ")
            && !ports.store.has_call_starting_with("publish "),
        "a failed digest read must not lock or publish: {:?}",
        ports.store.calls()
    );
}
