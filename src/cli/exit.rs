//! Process exit policy for parsed, operational, and forwarded outcomes.

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
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the exit-code partition.

    use rstest::rstest;

    use super::{Outcome, exit_code_for};

    #[rstest]
    #[case::success(Outcome::Success, 0)]
    #[case::display(Outcome::Display, 0)]
    #[case::usage_error(Outcome::UsageError, 2)]
    #[case::operational_failure(Outcome::OperationalFailure, 1)]
    #[case::forwarded_nonzero(Outcome::Forwarded(17), 17)]
    #[case::forwarded_windows_code(Outcome::Forwarded(256), 256)]
    fn maps_outcomes_to_exit_codes(#[case] outcome: Outcome, #[case] expected: i32) {
        assert_eq!(
            exit_code_for(&outcome),
            expected,
            "wrong exit code for {outcome:?}"
        );
    }
}
