//! The `whitaker check` exit-status contract, from ADR 0017.
//!
//! A lint run's own status passes through unchanged, because Dylint is the
//! authority on whether the code linted. Whitaker's own failures use `3` for a
//! configuration defect, `4` for provisioning and `5` for the host or the
//! execution of Dylint; none of those collides with a status `cargo dylint`
//! documents (`0`, `1`, `2`, `101`), with a signal-derived status, or with a
//! Windows process status.
//!
//! Pass-through is unconditional, so a Dylint run that itself exits `3`, `4`
//! or `5` returns a status inside Whitaker's range. A script that must tell
//! the two apart reads the reason key instead of the status: an operational
//! outcome prints `whitaker: error[<key>]`, and a pass-through prints nothing
//! of Whitaker's own. The status alone answers "did the code lint?", which is
//! the question Dylint's status was already answering.

use crate::ports::{HostFault, RepairableReason};

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
/// use whitaker::{
///     cli::check::exit::{CheckOutcome, check_exit_code},
///     ports::RepairableReason,
/// };
///
/// assert_eq!(
///     check_exit_code(&CheckOutcome::Linted(23)),
///     23,
///     "a linted outcome must exit with Dylint's own status",
/// );
/// assert_eq!(
///     check_exit_code(&CheckOutcome::Refused(RepairableReason::NotInstalled)),
///     4,
///     "a refusal must exit 4",
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
/// use whitaker::{
///     cli::check::exit::{CheckOutcome, reason_key},
///     ports::HostFault,
/// };
///
/// assert_eq!(
///     reason_key(&CheckOutcome::HostFailed(HostFault::DataDirectory)),
///     Some("host.data-dir"),
///     "an undeterminable data directory must report `host.data-dir`",
/// );
/// assert_eq!(
///     reason_key(&CheckOutcome::Linted(0)),
///     None,
///     "a successful lint must report no reason key",
/// );
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
    use crate::ports::{HostFault, RepairableReason};

    /// Map every outcome onto its exit code and its reason key.
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

    /// Keep every operational outcome inside Whitaker's own `3..=5` range.
    #[test]
    fn operational_failures_stay_in_the_whitaker_owned_range() {
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
    use crate::cli::check::{
        crate::ports::{HostFault, RepairableReason},
        flow::samplers::{any_fault, any_reason},
    };

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

    /// Check that every outcome maps to its status and carries a key exactly
    /// when it is operational.
    #[kani::proof]
    fn verify_check_exit_composition() {
        let code: i32 = kani::any();
        let outcome = any_outcome(code, kani::any());

        match outcome {
            CheckOutcome::Linted(own) => assert!(
                check_exit_code(&outcome) == own,
                "a linted outcome must exit with its own status"
            ),
            CheckOutcome::ConfigurationFailed => assert!(
                check_exit_code(&outcome) == 3,
                "a configuration failure must exit 3"
            ),
            CheckOutcome::Refused(_)
            | CheckOutcome::RepairFailed
            | CheckOutcome::NotConverged(_) => {
                assert!(
                    check_exit_code(&outcome) == 4,
                    "a provisioning failure must exit 4"
                );
            }
            CheckOutcome::HostFailed(_) | CheckOutcome::RunFailed | CheckOutcome::OutputFailed => {
                assert!(
                    check_exit_code(&outcome) == 5,
                    "an execution failure must exit 5"
                );
            }
        }
        match outcome {
            CheckOutcome::Linted(_) | CheckOutcome::ConfigurationFailed => {
                assert!(
                    reason_key(&outcome).is_none(),
                    "a linted or configuration outcome must carry no reason key"
                );
            }
            _ => assert!(
                reason_key(&outcome).is_some(),
                "every operational failure must carry a reason key"
            ),
        }
    }

    /// Define one reachability witness from an outcome and the two messages
    /// the gate matches.
    ///
    /// The messages must stay literals at the call site: `scripts/run-kani.sh`
    /// matches the witness description and the named failure, so a formatted
    /// message would not be found.
    macro_rules! exit_witness {
        (
            $(#[$meta:meta])*
            $name:ident,
            $outcome:expr,
            $status:literal,
            $witness:literal,
            $failure:literal,
        ) => {
            $(#[$meta])*
            #[kani::proof]
            fn $name() {
                assert!(check_exit_code(&$outcome) == $status, $witness);
                assert!(false, $failure);
            }
        };
    }

    exit_witness!(
        /// Witness a 0 exit for a linted outcome before failing deliberately.
        verify_linted_exit_is_reachable,
        CheckOutcome::Linted(0),
        0,
        "a linted outcome must exit 0",
        "the linted class should have a counterexample",
    );

    exit_witness!(
        /// Witness a 3 exit for the reserved configuration outcome.
        ///
        /// No flow reaches this outcome in phase one — 3.5.4 owns the flags —
        /// so the witness establishes that the arm is inhabited and maps to
        /// `3`, not that a user can trigger it today.
        verify_configuration_failed_exit_is_reachable,
        CheckOutcome::ConfigurationFailed,
        3,
        "a configuration failure must exit 3",
        "the configuration class should have a counterexample",
    );

    exit_witness!(
        /// Witness a 4 exit for a refusal before failing deliberately.
        verify_refused_exit_is_reachable,
        CheckOutcome::Refused(RepairableReason::NotInstalled),
        4,
        "a refused outcome must exit 4",
        "the refused class should have a counterexample",
    );

    exit_witness!(
        /// Witness a 4 exit for a failed repair before failing deliberately.
        verify_repair_failed_exit_is_reachable,
        CheckOutcome::RepairFailed,
        4,
        "a failed repair must exit 4",
        "the failed repair class should have a counterexample",
    );

    exit_witness!(
        /// Witness a 4 exit for a repair that left the installation unready.
        verify_not_converged_exit_is_reachable,
        CheckOutcome::NotConverged(RepairableReason::NotInstalled),
        4,
        "a non-converging repair must exit 4",
        "the not-converged class should have a counterexample",
    );

    exit_witness!(
        /// Witness a 5 exit for a run that could not start.
        verify_run_failed_exit_is_reachable,
        CheckOutcome::RunFailed,
        5,
        "a run that could not start must exit 5",
        "the run failure class should have a counterexample",
    );

    exit_witness!(
        /// Witness a 5 exit for a host failure before failing deliberately.
        verify_host_failed_exit_is_reachable,
        CheckOutcome::HostFailed(HostFault::Io),
        5,
        "a host failure must exit 5",
        "the host class should have a counterexample",
    );

    exit_witness!(
        /// Witness a 5 exit for an output failure before failing deliberately.
        verify_output_failed_exit_is_reachable,
        CheckOutcome::OutputFailed,
        5,
        "an output failure must exit 5",
        "the output class should have a counterexample",
    );
}
