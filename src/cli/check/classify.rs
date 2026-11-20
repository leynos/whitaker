//! Classifying a failed readiness check by what, if anything, can fix it.
//!
//! The distinction is the whole point of the repair flow: a missing or stale
//! artefact is provisioning work that an install can do, whereas an
//! undeterminable data directory or an unreadable filesystem is a host fault
//! that installing again cannot mend. Getting it backwards either loops a
//! failing install or hides a fixable installation behind an error.

use whitaker_installer::{bootstrap::BootstrapError, clippy_snapshot::ClippySnapshotError};

use crate::ports::CheckPreflightError;

/// Why an installation needs provisioning before it can lint.
///
/// The Kani derive lets the proof suites range over the real variant set, so a
/// new reason joins the proofs without a sampler table to update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum RepairableReason {
    /// No bootstrap metadata has been recorded.
    NotInstalled,
    /// The recorded metadata cannot be used.
    Metadata,
    /// The recorded lint-library directory is gone.
    LibrariesMissing,
    /// The recorded lint-library directory names more than one suite.
    LibrariesAmbiguous,
    /// The Clippy snapshot is missing or invalid.
    Snapshot,
    /// `cargo dylint` could not be queried.
    CargoDylint,
    /// The installed `cargo-dylint` is not the release this installation was
    /// built for.
    DylintVersion,
    /// The user managed the library path, and it names no single Whitaker
    /// suite: either none is reachable, or Dylint would have to choose between
    /// several and exits `1` when it cannot.
    UserPath,
}

impl RepairableReason {
    /// Return the bounded label this reason records.
    ///
    /// A label is not the reason key: keys are a user-facing contract, while
    /// labels name a metric series. Both are closed sets, so neither depends
    /// on a path or an error string.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::cli::check::classify::RepairableReason;
    ///
    /// assert_eq!(
    ///     RepairableReason::NotInstalled.label(),
    ///     "not_installed",
    ///     "a missing installation must label its metric series `not_installed`",
    /// );
    /// ```
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::Metadata => "metadata",
            Self::LibrariesMissing => "libraries_missing",
            Self::LibrariesAmbiguous => "libraries_ambiguous",
            Self::Snapshot => "snapshot",
            Self::CargoDylint => "cargo_dylint",
            Self::DylintVersion => "dylint_version",
            Self::UserPath => "user_path",
        }
    }
}

/// A host fault that installing again cannot fix.
///
/// Each variant carries the bounded label its telemetry records, so a host
/// fault is grouped without the label following an error message or a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum HostFault {
    /// The platform data directory could not be determined.
    DataDirectory,
    /// The filesystem failed while inspecting the installation.
    Io,
    /// The installation lock could not be taken.
    Lock,
    /// `cargo dylint` could not be started.
    Spawn,
}

impl HostFault {
    /// Return the bounded label this fault records.
    ///
    /// The label names a metric series, not a user-facing reason key, so it is
    /// a closed set rather than a rendering of the error.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::cli::check::classify::HostFault;
    ///
    /// assert_eq!(
    ///     HostFault::Lock.label(),
    ///     "lock",
    ///     "a lock that could not be taken must label its series `lock`",
    /// );
    /// ```
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DataDirectory => "data_directory",
            Self::Io => "io",
            Self::Lock => "lock",
            Self::Spawn => "spawn",
        }
    }
}

/// The outcome of a readiness check.
#[derive(Debug)]
pub enum Preflight<P> {
    /// Dylint can run against this validated value.
    Ready(P),
    /// The installation is incomplete or stale, and provisioning may fix it.
    Repairable(RepairableReason),
    /// The host failed in a way provisioning cannot fix.
    Host(HostFault),
}

/// A failed readiness check, classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightFailure {
    /// Provisioning may fix the installation.
    Repairable(RepairableReason),
    /// The host itself is at fault.
    Host(HostFault),
}

impl PreflightFailure {
    /// Convert a classified installation error into a preflight result.
    ///
    /// The `Ok` value is unreachable, because a failure carries no prepared
    /// installation; this keeps the two representations convertible in one
    /// direction only.
    #[must_use]
    pub const fn into_preflight<P>(self) -> Preflight<P> {
        match self {
            Self::Repairable(reason) => Preflight::Repairable(reason),
            Self::Host(fault) => Preflight::Host(fault),
        }
    }
}

/// Classify a bootstrap failure by what can fix it.
///
/// Readiness does start one child — `cargo dylint --version` — so a
/// [`BootstrapError::Spawn`] is reachable here. It means `cargo` itself could
/// not be run, which installing Whitaker cannot mend, so it is a host fault.
///
/// # Examples
///
/// ```
/// use whitaker::cli::check::classify::{HostFault, PreflightFailure, classify_failure};
/// use whitaker_installer::bootstrap::BootstrapError;
///
/// assert_eq!(
///     classify_failure(&BootstrapError::NoDataDirectory),
///     PreflightFailure::Host(HostFault::DataDirectory),
///     "an undeterminable data directory must be a host fault",
/// );
/// ```
#[must_use]
pub const fn classify_failure(error: &BootstrapError) -> PreflightFailure {
    match error {
        BootstrapError::NotInstalled { .. } => {
            PreflightFailure::Repairable(RepairableReason::NotInstalled)
        }
        BootstrapError::Metadata { .. } => PreflightFailure::Repairable(RepairableReason::Metadata),
        BootstrapError::LibrariesMissing { .. } => {
            PreflightFailure::Repairable(RepairableReason::LibrariesMissing)
        }
        // A snapshot that exists but cannot be read is the filesystem's
        // failure, not the installation's: installing again cannot fix a
        // denied directory. Everything else the probe rejects is a defect a
        // reinstall can replace.
        BootstrapError::Snapshot(ClippySnapshotError::Io { .. }) => {
            PreflightFailure::Host(HostFault::Io)
        }
        BootstrapError::Snapshot(_) => PreflightFailure::Repairable(RepairableReason::Snapshot),
        BootstrapError::CargoDylint { .. } => {
            PreflightFailure::Repairable(RepairableReason::CargoDylint)
        }
        BootstrapError::DylintVersion { .. } => {
            PreflightFailure::Repairable(RepairableReason::DylintVersion)
        }
        BootstrapError::NoDataDirectory => PreflightFailure::Host(HostFault::DataDirectory),
        BootstrapError::Io { .. } => PreflightFailure::Host(HostFault::Io),
        BootstrapError::Spawn { .. } => PreflightFailure::Host(HostFault::Spawn),
    }
}

/// Classify a failed readiness check, including the user-path case.
///
/// The user managed the library path themselves, so an empty or ambiguous one
/// is repairable only by them: installing Whitaker elsewhere cannot change a
/// directory they chose.
///
/// # Examples
///
/// ```
/// use whitaker::{
///     cli::check::classify::{HostFault, PreflightFailure, classify_preflight_failure},
///     ports::CheckPreflightError,
/// };
///
/// assert_eq!(
///     classify_preflight_failure(&CheckPreflightError::UserPathHasNoSuite),
///     PreflightFailure::Repairable(whitaker::cli::check::classify::RepairableReason::UserPath),
///     "a user-managed path with no suite must be repairable by the user",
/// );
/// ```
#[must_use]
pub const fn classify_preflight_failure(error: &CheckPreflightError) -> PreflightFailure {
    match error {
        CheckPreflightError::Bootstrap(failure) => classify_failure(failure),
        CheckPreflightError::UserPathHasNoSuite | CheckPreflightError::UserPathAmbiguous => {
            PreflightFailure::Repairable(RepairableReason::UserPath)
        }
        // The directory is the installation's own, so a repair is the first
        // thing to try; the flow reports `NotConverged` if installing does not
        // replace the extra library.
        CheckPreflightError::RecordedPathAmbiguous { .. } => {
            PreflightFailure::Repairable(RepairableReason::LibrariesAmbiguous)
        }
    }
}

#[cfg(test)]
mod tests {
    //! The classification table, one case per bootstrap error variant.

    use rstest::rstest;
    use whitaker_installer::{bootstrap::BootstrapError, clippy_snapshot::ClippySnapshotError};

    use super::{
        HostFault,
        PreflightFailure,
        RepairableReason,
        classify_failure,
        classify_preflight_failure,
    };
    use crate::ports::CheckPreflightError;

    /// Classify every bootstrap error variant into its expected preflight class.
    #[rstest]
    #[case::not_installed(
        BootstrapError::NotInstalled { path: "bootstrap.json".to_owned() },
        PreflightFailure::Repairable(RepairableReason::NotInstalled)
    )]
    #[case::metadata(
        BootstrapError::Metadata { path: "bootstrap.json".to_owned(), reason: "bad".to_owned() },
        PreflightFailure::Repairable(RepairableReason::Metadata)
    )]
    #[case::libraries_missing(
        BootstrapError::LibrariesMissing { path: "lints".to_owned() },
        PreflightFailure::Repairable(RepairableReason::LibrariesMissing)
    )]
    #[case::snapshot(
        BootstrapError::Snapshot(ClippySnapshotError::Manifest { reason: "bad".to_owned() }),
        PreflightFailure::Repairable(RepairableReason::Snapshot)
    )]
    #[case::snapshot_io(
        BootstrapError::Snapshot(ClippySnapshotError::Io {
            path: "snapshot".to_owned(),
            reason: "denied".to_owned(),
        }),
        PreflightFailure::Host(HostFault::Io)
    )]
    #[case::cargo_dylint(
        BootstrapError::CargoDylint { reason: "missing".to_owned() },
        PreflightFailure::Repairable(RepairableReason::CargoDylint)
    )]
    #[case::dylint_version(
        BootstrapError::DylintVersion { expected: "6.3.0".to_owned(), found: "6.2.0".to_owned() },
        PreflightFailure::Repairable(RepairableReason::DylintVersion)
    )]
    #[case::no_data_directory(
        BootstrapError::NoDataDirectory,
        PreflightFailure::Host(HostFault::DataDirectory)
    )]
    #[case::io(
        BootstrapError::Io { path: "lints".to_owned(), reason: "denied".to_owned() },
        PreflightFailure::Host(HostFault::Io)
    )]
    #[case::spawn(
        BootstrapError::Spawn { reason: "missing".to_owned() },
        PreflightFailure::Host(HostFault::Spawn)
    )]
    fn every_bootstrap_error_has_a_class(
        #[case] error: BootstrapError,
        #[case] expected: PreflightFailure,
    ) {
        assert_eq!(
            classify_failure(&error),
            expected,
            "wrong preflight class for {error}"
        );
    }

    /// Keep a recorded directory that names two suites repairable.
    #[test]
    fn a_recorded_ambiguous_directory_is_repairable() {
        let failure = classify_preflight_failure(&CheckPreflightError::RecordedPathAmbiguous {
            path: "lints/lib".to_owned(),
        });

        assert_eq!(
            failure,
            PreflightFailure::Repairable(RepairableReason::LibrariesAmbiguous),
            "a recorded directory holding two suites must be repairable, not a host fault"
        );
    }

    /// Keep a user-managed path failure repairable, and only by the user.
    #[test]
    fn a_user_path_failure_is_a_repairable_reason() {
        // The reason has no bootstrap error behind it: the readiness check
        // produced it from the user's own library path, and only the user can
        // change that.
        let failure = classify_preflight_failure(&CheckPreflightError::UserPathHasNoSuite);

        assert_eq!(
            failure,
            PreflightFailure::Repairable(RepairableReason::UserPath),
            "a user path failure must be repairable, and by the user alone"
        );
        assert!(
            matches!(
                failure.into_preflight::<()>(),
                super::Preflight::Repairable(RepairableReason::UserPath)
            ),
            "a user path failure must stay repairable in the preflight result"
        );
    }

    /// Leave bootstrap classification unchanged when routed through the wrapper.
    #[test]
    fn a_bootstrap_failure_is_classified_through_its_own_table() {
        let failure = classify_preflight_failure(&CheckPreflightError::Bootstrap(
            BootstrapError::NotInstalled {
                path: "bootstrap.json".to_owned(),
            },
        ));

        assert_eq!(
            failure,
            PreflightFailure::Repairable(RepairableReason::NotInstalled),
            "the wrapper must not change how a bootstrap failure is classified"
        );
    }
}
