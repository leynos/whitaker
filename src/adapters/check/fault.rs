//! Translating installer failures into the `check` port's own faults.
//!
//! `BootstrapError` is the installer's report, and it belongs to the crate that
//! detected the defect. The port describes the same defects in
//! [`BootstrapFault`], so the command layer never names an installer type and
//! a later change to the installer's vocabulary cannot reach the classifier.
//! This module is the one place that knows both types, and every other module
//! in the adapter works with the port's.

use whitaker_installer::{bootstrap::BootstrapError, clippy_snapshot::ClippySnapshotError};

use crate::ports::BootstrapFault;

/// Translate an installer failure into the port's fault type.
///
/// The mapping is one-to-one except for the snapshot, whose unreadable case
/// becomes [`BootstrapFault::SnapshotIo`]: a denied or failed read is the
/// host's defect, and installing again cannot repair permissions or a disk.
/// The pair is exhaustive, so a variant the installer adds fails to compile
/// here rather than reaching the classifier as an unhandled shape.
pub(super) fn fault_of(error: BootstrapError) -> BootstrapFault {
    match error {
        BootstrapError::NoDataDirectory => BootstrapFault::NoDataDirectory,
        BootstrapError::NotInstalled { path } => BootstrapFault::NotInstalled { path },
        BootstrapError::Metadata { path, reason } => BootstrapFault::Metadata { path, reason },
        BootstrapError::LibrariesMissing { path } => BootstrapFault::LibrariesMissing { path },
        BootstrapError::Snapshot(snapshot) => snapshot_fault(snapshot),
        BootstrapError::CargoDylint { reason } => BootstrapFault::CargoDylint { reason },
        BootstrapError::DylintVersion { expected, found } => {
            BootstrapFault::DylintVersion { expected, found }
        }
        BootstrapError::Io { path, reason } => BootstrapFault::Io { path, reason },
        BootstrapError::Spawn { reason } => BootstrapFault::Spawn { reason },
    }
}

/// Translate a snapshot failure, keeping the unreadable-snapshot distinction.
///
/// Every other snapshot defect describes an installation a reinstall can
/// replace, so it keeps the installer's own sentence as the reason a
/// repairable fault carries.
fn snapshot_fault(error: ClippySnapshotError) -> BootstrapFault {
    match error {
        ClippySnapshotError::Io { path, reason } => BootstrapFault::SnapshotIo { path, reason },
        other => BootstrapFault::Snapshot {
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    //! The translation table, one case per installer failure shape.

    use rstest::rstest;
    use whitaker_installer::{bootstrap::BootstrapError, clippy_snapshot::ClippySnapshotError};

    use super::fault_of;
    use crate::ports::BootstrapFault;

    /// Translate every installer failure into the port's own fault.
    #[rstest]
    #[case::not_installed(
        BootstrapError::NotInstalled { path: "bootstrap.json".to_owned() },
        BootstrapFault::NotInstalled { path: "bootstrap.json".to_owned() }
    )]
    #[case::metadata(
        BootstrapError::Metadata { path: "bootstrap.json".to_owned(), reason: "bad".to_owned() },
        BootstrapFault::Metadata { path: "bootstrap.json".to_owned(), reason: "bad".to_owned() }
    )]
    #[case::libraries_missing(
        BootstrapError::LibrariesMissing { path: "lints".to_owned() },
        BootstrapFault::LibrariesMissing { path: "lints".to_owned() }
    )]
    #[case::snapshot(
        BootstrapError::Snapshot(ClippySnapshotError::Manifest { reason: "bad".to_owned() }),
        BootstrapFault::Snapshot {
            reason: ClippySnapshotError::Manifest { reason: "bad".to_owned() }.to_string(),
        }
    )]
    #[case::snapshot_io(
        BootstrapError::Snapshot(ClippySnapshotError::Io {
            path: "snapshot".to_owned(),
            reason: "denied".to_owned(),
        }),
        BootstrapFault::SnapshotIo { path: "snapshot".to_owned(), reason: "denied".to_owned() }
    )]
    #[case::cargo_dylint(
        BootstrapError::CargoDylint { reason: "missing".to_owned() },
        BootstrapFault::CargoDylint { reason: "missing".to_owned() }
    )]
    #[case::dylint_version(
        BootstrapError::DylintVersion { expected: "6.3.0".to_owned(), found: "6.2.0".to_owned() },
        BootstrapFault::DylintVersion { expected: "6.3.0".to_owned(), found: "6.2.0".to_owned() }
    )]
    #[case::no_data_directory(BootstrapError::NoDataDirectory, BootstrapFault::NoDataDirectory)]
    #[case::io(
        BootstrapError::Io { path: "lints".to_owned(), reason: "denied".to_owned() },
        BootstrapFault::Io { path: "lints".to_owned(), reason: "denied".to_owned() }
    )]
    #[case::spawn(
        BootstrapError::Spawn { reason: "missing".to_owned() },
        BootstrapFault::Spawn { reason: "missing".to_owned() }
    )]
    fn every_installer_failure_becomes_a_port_fault(
        #[case] error: BootstrapError,
        #[case] expected: BootstrapFault,
    ) {
        assert_eq!(
            fault_of(error),
            expected,
            "an installer failure must keep its meaning across the port boundary"
        );
    }

    /// Keep a repairable snapshot's own sentence in the translated reason.
    #[test]
    fn a_rejected_snapshot_keeps_the_installers_explanation() {
        let error = ClippySnapshotError::BundleMissing {
            path: "/bundles/clippy.tar.zst".to_owned(),
            asset: "clippy.tar.zst".to_owned(),
        };

        let translated = fault_of(BootstrapError::Snapshot(error.clone()));

        assert_eq!(
            translated,
            BootstrapFault::Snapshot {
                reason: error.to_string()
            },
            "a reinstallable snapshot defect must carry the installer's own sentence"
        );
    }
}
