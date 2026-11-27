//! Assembling the `cargo dylint` argument vector for `whitaker check`.
//!
//! `check` owns a small, closed set of Dylint options and forwards the rest of
//! the command line untouched. Keeping the owned set a closed enum is what
//! makes the forwarding safe: every Whitaker token renders as one argument and
//! none of them is `--`, so the first `--` in the assembled vector is always
//! Whitaker's own separator and everything after it is the user's.

use std::ffi::OsString;

use super::cargo::CargoOptions;
use crate::library_path::SUITE_CRATE;

/// A Dylint option Whitaker owns for a `check` run.
///
/// The set is closed deliberately. `--all` is excluded because it loads every
/// discovered library, so a path with no suite would exit successfully having
/// linted nothing, and `--no-deps` is excluded because `check` should cover
/// the workspace as `cargo check` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DylintOption {
    /// Load exactly the Whitaker suite library.
    Lib,
    /// Ignore libraries discovered in the workspace metadata.
    ///
    /// Without it `--lib` also searches the project's build output, so a
    /// project that happens to have built its own `whitaker_suite` would run
    /// that library instead of the recorded one.
    NoMetadata,
}

impl DylintOption {
    /// The suite library [`Self::Lib`] names.
    pub const LIBRARY: &str = SUITE_CRATE;

    /// The options `check` passes, in the order it passes them.
    pub const SET: [Self; 2] = [Self::Lib, Self::NoMetadata];

    /// Render this option as the single argument it becomes.
    ///
    /// `--lib=NAME` rather than `--lib NAME` so that no Whitaker-owned
    /// argument can ever equal `--`.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::cli::check::argv::DylintOption;
    ///
    /// assert_eq!(
    ///     DylintOption::Lib.token(),
    ///     "--lib=whitaker_suite",
    ///     "`--lib` must render as the single `--lib=whitaker_suite` token",
    /// );
    /// assert_eq!(
    ///     DylintOption::NoMetadata.token(),
    ///     "--no-metadata",
    ///     "`--no-metadata` must render as itself",
    /// );
    /// ```
    #[must_use]
    pub fn token(self) -> OsString {
        match self {
            Self::Lib => OsString::from(format!("--lib={}", Self::LIBRARY)),
            Self::NoMetadata => OsString::from("--no-metadata"),
        }
    }
}

/// Assemble the arguments `cargo dylint` receives for a `check` run.
///
/// The result is `W ++ ["--"] ++ U`: Whitaker's options, its separator, then
/// the Cargo selectors Whitaker re-emits and the user's forwarded arguments
/// byte for byte. A `--` inside `U` survives, because only the first separator
/// splits the vector.
///
/// # Examples
///
/// ```
/// use std::ffi::OsString;
///
/// use whitaker::cli::check::{CargoOptions, argv::assemble_dylint_args};
///
/// let options = CargoOptions {
///     packages: vec!["x".to_owned()],
///     ..CargoOptions::default()
/// };
/// let arguments = assemble_dylint_args(&options, &[OsString::from("-q")]);
/// assert_eq!(
///     arguments,
///     [
///         "--lib=whitaker_suite",
///         "--no-metadata",
///         "--",
///         "-p",
///         "x",
///         "-q"
///     ],
///     "the owned options and separator must precede every Cargo argument",
/// );
/// ```
#[must_use]
pub fn assemble_dylint_args(options: &CargoOptions, cargo_args: &[OsString]) -> Vec<OsString> {
    DylintOption::SET
        .iter()
        .map(|option| option.token())
        .chain(std::iter::once(OsString::from("--")))
        .chain(options.tokens())
        .chain(cargo_args.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    //! Tests for the single-token and separator guarantees.

    use std::ffi::{OsStr, OsString};

    use googletest::prelude::*;
    use rstest::rstest;

    use super::{DylintOption, assemble_dylint_args};
    use crate::cli::check::CargoOptions;

    /// Render each owned option as one non-empty, non-separator argument.
    #[rstest]
    #[case::lib(DylintOption::Lib)]
    #[case::no_metadata(DylintOption::NoMetadata)]
    fn every_owned_option_is_one_argument_that_is_not_a_separator(
        #[case] option: DylintOption,
    ) -> googletest::Result<()> {
        let token = option.token();

        verify_that!(token.is_empty(), eq(false))?;
        verify_that!(token == OsStr::new("--"), eq(false))?;
        verify_that!(
            token.to_string_lossy().contains(char::is_whitespace),
            eq(false)
        )?;
        Ok(())
    }

    /// Place Whitaker's separator after its options, keeping the user's own.
    #[test]
    fn the_assembled_vector_puts_the_separator_after_the_owned_options() {
        let user_arguments = [
            OsString::from("-p"),
            OsString::from("--"),
            OsString::from("-q"),
        ];

        let assembled = assemble_dylint_args(&CargoOptions::default(), &user_arguments);

        assert_eq!(
            assembled,
            [
                "--lib=whitaker_suite",
                "--no-metadata",
                "--",
                "-p",
                "--",
                "-q"
            ],
            "the first separator must be Whitaker's own"
        );
    }

    /// End the vector with the separator even when the user passes nothing.
    #[test]
    fn an_empty_user_suffix_still_ends_with_the_separator() {
        let assembled = assemble_dylint_args(&CargoOptions::default(), &[]);

        assert_eq!(
            assembled,
            ["--lib=whitaker_suite", "--no-metadata", "--"],
            "the separator must always be present"
        );
    }
}
