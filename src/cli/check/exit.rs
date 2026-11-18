//! The `whitaker check` exit-status contract, from ADR 0017.
//!
//! A lint run's own status passes through unchanged, because Dylint is the
//! authority on whether the code linted. Whitaker's own failures use a small
//! set of codes disjoint from every status `cargo dylint` documents, so a
//! script can tell "the code failed" from "Whitaker could not try": fix the
//! configuration (`3`), provision (`4`), or investigate the host (`5`).

use super::classify::{HostFault, RepairableReason};

/// How a `whitaker check` invocation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Dylint ran and reported this status.
    Linted(i32),
    /// The configuration is invalid. Reserved for 3.5.4.
    ConfigurationFailed,
    /// Provisioning was needed and was not permitted.
    Refused(RepairableReason),
    /// Provisioning ran and failed.
    RepairFailed,
    /// Provisioning ran and the installation is still not ready.
    NotConverged(RepairableReason),
    /// The host failed in a way provisioning cannot fix.
    HostFailed(HostFault),
    /// Dylint could not be started.
    RunFailed,
    /// Whitaker could not write its own output.
    OutputFailed,
}

/// Map an outcome onto the process exit code.
///
/// # Examples
///
/// ```
/// use whitaker::cli::check::{
///     classify::RepairableReason,
///     exit::{CheckOutcome, check_exit_code},
/// };
///
/// assert_eq!(check_exit_code(&CheckOutcome::Linted(23)), 23);
/// assert_eq!(
///     check_exit_code(&CheckOutcome::Refused(RepairableReason::NotInstalled)),
///     4,
/// );
/// ```
#[must_use]
pub const fn check_exit_code(outcome: &CheckOutcome) -> i32 {
    match outcome {
        CheckOutcome::Linted(code) => *code,
        CheckOutcome::ConfigurationFailed => 3,
        CheckOutcome::Refused(_) | CheckOutcome::RepairFailed | CheckOutcome::NotConverged(_) => 4,
        CheckOutcome::HostFailed(_) | CheckOutcome::RunFailed | CheckOutcome::OutputFailed => 5,
    }
}

/// The stable, untranslated reason key an operational failure reports.
///
/// A key is a contract with scripts: it must not change when the prose around
/// it does, so it is drawn from a closed set rather than built from the error.
///
/// # Examples
///
/// ```
/// use whitaker::cli::check::{
///     classify::HostFault,
///     exit::{CheckOutcome, reason_key},
/// };
///
/// assert_eq!(
///     reason_key(&CheckOutcome::HostFailed(HostFault::DataDirectory)),
///     Some("host.data-dir"),
/// );
/// assert_eq!(reason_key(&CheckOutcome::Linted(0)), None);
/// ```
#[must_use]
pub const fn reason_key(outcome: &CheckOutcome) -> Option<&'static str> {
    match outcome {
        CheckOutcome::Linted(_) | CheckOutcome::ConfigurationFailed => None,
        CheckOutcome::Refused(RepairableReason::UserPath) => Some("provision.user-path"),
        CheckOutcome::Refused(_) => Some("provision.refused"),
        CheckOutcome::RepairFailed => Some("provision.repair-failed"),
        CheckOutcome::NotConverged(_) => Some("provision.not-converged"),
        CheckOutcome::HostFailed(HostFault::DataDirectory) => Some("host.data-dir"),
        CheckOutcome::HostFailed(HostFault::Io) => Some("host.io"),
        CheckOutcome::HostFailed(HostFault::Lock) => Some("host.lock"),
        CheckOutcome::HostFailed(HostFault::Spawn) | CheckOutcome::RunFailed => Some("exec.spawn"),
        CheckOutcome::OutputFailed => Some("exec.output"),
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the exit-code and reason-key partitions.

    use rstest::rstest;

    use super::{CheckOutcome, check_exit_code, reason_key};
    use crate::cli::check::classify::{HostFault, RepairableReason};

    #[rstest]
    #[case::success(CheckOutcome::Linted(0), 0, None)]
    #[case::lint_denied(CheckOutcome::Linted(1), 1, None)]
    #[case::dylint_usage(CheckOutcome::Linted(2), 2, None)]
    #[case::dylint_panic(CheckOutcome::Linted(101), 101, None)]
    #[case::signalled(CheckOutcome::Linted(143), 143, None)]
    #[case::windows_status(CheckOutcome::Linted(-1_073_741_819), -1_073_741_819, None)]
    #[case::configuration(CheckOutcome::ConfigurationFailed, 3, None)]
    #[case::refused(
        CheckOutcome::Refused(RepairableReason::NotInstalled),
        4,
        Some("provision.refused")
    )]
    #[case::refused_user_path(
        CheckOutcome::Refused(RepairableReason::UserPath),
        4,
        Some("provision.user-path")
    )]
    #[case::repair_failed(CheckOutcome::RepairFailed, 4, Some("provision.repair-failed"))]
    #[case::not_converged(
        CheckOutcome::NotConverged(RepairableReason::Snapshot),
        4,
        Some("provision.not-converged")
    )]
    #[case::host_data_dir(
        CheckOutcome::HostFailed(HostFault::DataDirectory),
        5,
        Some("host.data-dir")
    )]
    #[case::host_io(CheckOutcome::HostFailed(HostFault::Io), 5, Some("host.io"))]
    #[case::host_lock(CheckOutcome::HostFailed(HostFault::Lock), 5, Some("host.lock"))]
    #[case::host_spawn(CheckOutcome::HostFailed(HostFault::Spawn), 5, Some("exec.spawn"))]
    #[case::run_failed(CheckOutcome::RunFailed, 5, Some("exec.spawn"))]
    #[case::output_failed(CheckOutcome::OutputFailed, 5, Some("exec.output"))]
    fn maps_outcomes_to_exit_codes_and_reason_keys(
        #[case] outcome: CheckOutcome,
        #[case] expected_code: i32,
        #[case] expected_key: Option<&str>,
    ) {
        assert_eq!(
            check_exit_code(&outcome),
            expected_code,
            "wrong exit code for {outcome:?}"
        );
        assert_eq!(
            reason_key(&outcome),
            expected_key,
            "wrong reason key for {outcome:?}"
        );
    }

    #[test]
    fn operational_failures_never_collide_with_dylint_statuses() {
        let operational = [
            CheckOutcome::ConfigurationFailed,
            CheckOutcome::Refused(RepairableReason::Metadata),
            CheckOutcome::RepairFailed,
            CheckOutcome::NotConverged(RepairableReason::CargoDylint),
            CheckOutcome::HostFailed(HostFault::Lock),
            CheckOutcome::RunFailed,
            CheckOutcome::OutputFailed,
        ];

        for outcome in operational {
            let code = check_exit_code(&outcome);
            assert!(
                (3..=5).contains(&code),
                "{outcome:?} must use a Whitaker-owned status, got {code}"
            );
        }
    }
}

#[cfg(kani)]
mod verification {
    //! Exhaustive checks for exit composition and reason-key totality.

    use super::{CheckOutcome, check_exit_code, reason_key};
    use crate::cli::check::classify::{HostFault, RepairableReason};

    /// Pick one of the seven repairable reasons.
    fn any_reason() -> RepairableReason {
        match kani::any::<u8>() {
            0 => RepairableReason::NotInstalled,
            1 => RepairableReason::Metadata,
            2 => RepairableReason::LibrariesMissing,
            3 => RepairableReason::Snapshot,
            4 => RepairableReason::CargoDylint,
            5 => RepairableReason::DylintVersion,
            _ => RepairableReason::UserPath,
        }
    }

    /// Pick one of the four host faults.
    fn any_fault() -> HostFault {
        match kani::any::<u8>() {
            0 => HostFault::DataDirectory,
            1 => HostFault::Io,
            2 => HostFault::Lock,
            _ => HostFault::Spawn,
        }
    }

    /// Build one of the eight outcome shapes.
    fn any_outcome(code: i32, selector: u8) -> CheckOutcome {
        match selector {
            0 => CheckOutcome::Linted(code),
            1 => CheckOutcome::ConfigurationFailed,
            2 => CheckOutcome::Refused(any_reason()),
            3 => CheckOutcome::RepairFailed,
            4 => CheckOutcome::NotConverged(any_reason()),
            5 => CheckOutcome::HostFailed(any_fault()),
            6 => CheckOutcome::RunFailed,
            _ => CheckOutcome::OutputFailed,
        }
    }

    #[kani::proof]
    fn verify_check_exit_composition() {
        let code: i32 = kani::any();
        let outcome = any_outcome(code, kani::any());

        match outcome {
            CheckOutcome::Linted(own) => assert!(check_exit_code(&outcome) == own),
            CheckOutcome::ConfigurationFailed => assert!(check_exit_code(&outcome) == 3),
            CheckOutcome::Refused(_)
            | CheckOutcome::RepairFailed
            | CheckOutcome::NotConverged(_) => {
                assert!(check_exit_code(&outcome) == 4);
            }
            CheckOutcome::HostFailed(_) | CheckOutcome::RunFailed | CheckOutcome::OutputFailed => {
                assert!(check_exit_code(&outcome) == 5);
            }
        }
        match outcome {
            CheckOutcome::Linted(_) | CheckOutcome::ConfigurationFailed => {
                assert!(reason_key(&outcome).is_none());
            }
            _ => assert!(
                reason_key(&outcome).is_some(),
                "every operational failure must carry a reason key"
            ),
        }
    }

    #[kani::proof]
    fn verify_linted_exit_is_reachable() {
        assert!(check_exit_code(&CheckOutcome::Linted(0)) == 0);
        assert!(false, "the linted class should have a counterexample");
    }

    #[kani::proof]
    fn verify_refused_exit_is_reachable() {
        assert!(check_exit_code(&CheckOutcome::Refused(RepairableReason::NotInstalled)) == 4);
        assert!(false, "the refused class should have a counterexample");
    }

    #[kani::proof]
    fn verify_host_failed_exit_is_reachable() {
        assert!(check_exit_code(&CheckOutcome::HostFailed(HostFault::Io)) == 5);
        assert!(false, "the host class should have a counterexample");
    }

    #[kani::proof]
    fn verify_output_failed_exit_is_reachable() {
        assert!(check_exit_code(&CheckOutcome::OutputFailed) == 5);
        assert!(false, "the output class should have a counterexample");
    }
}
