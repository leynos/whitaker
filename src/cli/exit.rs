//! Process exit policy for parsed, operational, and forwarded outcomes.

use super::check::exit::{CheckOutcome, check_exit_code};

/// A completed command outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A command completed successfully.
    Success,
    /// A help or version request was printed.
    Display,
    /// Argument parsing failed.
    UsageError,
    /// An installer, inventory, output, or process error occurred.
    OperationalFailure,
    /// `cargo dylint` returned this status.
    Forwarded(i32),
    /// `whitaker check` finished, and owns its own status contract.
    Check(CheckOutcome),
}

/// Maps an outcome onto a process exit code.
///
/// # Examples
///
/// ```
/// use whitaker::cli::exit::{Outcome, exit_code_for};
///
/// assert_eq!(exit_code_for(&Outcome::UsageError), 2);
/// assert_eq!(exit_code_for(&Outcome::Forwarded(23)), 23);
/// ```
#[must_use]
pub const fn exit_code_for(outcome: &Outcome) -> i32 {
    match outcome {
        Outcome::Success | Outcome::Display => 0,
        Outcome::UsageError => 2,
        Outcome::OperationalFailure => 1,
        Outcome::Forwarded(code) => *code,
        Outcome::Check(check) => check_exit_code(check),
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the exit-code partition.

    use rstest::rstest;

    use super::{Outcome, exit_code_for};
    use crate::cli::check::{classify::RepairableReason, exit::CheckOutcome};

    #[rstest]
    #[case::success(Outcome::Success, 0)]
    #[case::display(Outcome::Display, 0)]
    #[case::usage_error(Outcome::UsageError, 2)]
    #[case::operational_failure(Outcome::OperationalFailure, 1)]
    #[case::forwarded_nonzero(Outcome::Forwarded(17), 17)]
    #[case::forwarded_windows_code(Outcome::Forwarded(256), 256)]
    #[case::check_linted(Outcome::Check(CheckOutcome::Linted(1)), 1)]
    // A non-default child status proves the check route passes the status
    // through rather than normalizing every non-zero value to one.
    #[case::check_linted_passthrough(Outcome::Check(CheckOutcome::Linted(101)), 101)]
    #[case::check_refused(
        Outcome::Check(CheckOutcome::Refused(RepairableReason::NotInstalled)),
        4
    )]
    #[case::check_host_failed(Outcome::Check(CheckOutcome::RunFailed), 5)]
    fn maps_outcomes_to_exit_codes(#[case] outcome: Outcome, #[case] expected: i32) {
        assert_eq!(
            exit_code_for(&outcome),
            expected,
            "wrong exit code for {outcome:?}"
        );
    }
}
