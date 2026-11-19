//! Pure token classification for the root command and legacy Dylint forwarding.

/// Token categories relevant to command selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenTag {
    /// The `install` subcommand.
    Install,
    /// The `ls` subcommand.
    Ls,
    /// The `check` subcommand.
    Check,
    /// A help request.
    Help,
    /// A version request.
    Version,
    /// The end-of-options marker.
    DoubleDash,
    /// A root-level quiet or verbose option.
    GlobalFlag,
    /// An option owned by `cargo dylint`.
    DylintFlag,
    /// A value or unrecognized token.
    Other,
}

/// A recognized Whitaker command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subcommand {
    /// Install dependencies and lint libraries.
    Install,
    /// List installed lint libraries.
    Ls,
    /// Lint the current package with the Whitaker suite.
    Check,
}

/// How an argument vector should be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Dispatch to a Whitaker subcommand.
    Subcommand(Subcommand),
    /// Forward verbatim to `cargo dylint`.
    Forward,
    /// Parse and print root help or version output.
    Display,
}

/// Classifies a tokenized argument vector.
///
/// Leading shared quiet and verbose flags do not change the command. The first
/// other token decides routing, so Dylint options remain owned by the legacy
/// forwarding path even when they precede a subcommand-looking value.
///
/// # Examples
///
/// ```
/// use whitaker::cli::dispatch::{Class, Subcommand, TokenTag, classify};
///
/// assert_eq!(
///     classify(&[TokenTag::GlobalFlag, TokenTag::Install]),
///     Class::Subcommand(Subcommand::Install),
///     "a leading quiet flag must not change the selected subcommand",
/// );
/// assert_eq!(
///     classify(&[TokenTag::DylintFlag]),
///     Class::Forward,
///     "a Dylint option must reach the legacy forwarding path",
/// );
/// ```
#[must_use]
pub fn classify(tokens: &[TokenTag]) -> Class {
    for token in tokens {
        match token {
            TokenTag::GlobalFlag => {}
            TokenTag::Install => return Class::Subcommand(Subcommand::Install),
            TokenTag::Ls => return Class::Subcommand(Subcommand::Ls),
            TokenTag::Check => return Class::Subcommand(Subcommand::Check),
            TokenTag::Help | TokenTag::Version => return Class::Display,
            TokenTag::DoubleDash | TokenTag::DylintFlag | TokenTag::Other => {
                return Class::Forward;
            }
        }
    }
    Class::Forward
}

#[cfg(test)]
mod tests {
    //! Parameterized examples for the routing precedence.

    use googletest::prelude::*;
    use rstest::rstest;

    use super::{Class, Subcommand, TokenTag, classify};

    /// Select every recognized command rather than forwarding it.
    #[gtest]
    #[rstest]
    #[case::install(Subcommand::Install, vec![TokenTag::Install])]
    #[case::install_after_quiet(Subcommand::Install, vec![TokenTag::GlobalFlag, TokenTag::Install])]
    #[case::ls(Subcommand::Ls, vec![TokenTag::Ls])]
    #[case::check(Subcommand::Check, vec![TokenTag::Check])]
    #[case::check_after_verbosity(
        Subcommand::Check,
        vec![TokenTag::GlobalFlag, TokenTag::Check]
    )]
    fn recognized_commands_are_never_forwarded(
        #[case] subcommand: Subcommand,
        #[case] tokens: Vec<TokenTag>,
    ) -> googletest::Result<()> {
        verify_that!(classify(&tokens), eq(Class::Subcommand(subcommand)))?;
        Ok(())
    }

    /// Display help or version output without forwarding to Dylint.
    #[rstest]
    #[case::help(vec![TokenTag::Help])]
    #[case::version(vec![TokenTag::GlobalFlag, TokenTag::Version])]
    fn display_requests_are_separated_from_dylint_options(#[case] tokens: Vec<TokenTag>) {
        assert_eq!(
            classify(&tokens),
            Class::Display,
            "display request must not forward"
        );
    }

    /// Forward Dylint options and unknown arguments unchanged.
    #[rstest]
    #[case::all(vec![TokenTag::DylintFlag])]
    #[case::literal_after_double_dash(vec![TokenTag::DoubleDash, TokenTag::Other])]
    #[case::unrecognized_command(vec![TokenTag::Other])]
    #[case::empty(vec![])]
    fn dylint_and_unknown_arguments_are_forwarded(#[case] tokens: Vec<TokenTag>) {
        assert_eq!(
            classify(&tokens),
            Class::Forward,
            "legacy arguments must forward"
        );
    }
}

#[cfg(kani)]
mod verification {
    //! Bounded checks for routing totality, disjointness, and reachability.

    use super::{Class, Subcommand, TokenTag, classify};

    /// The number of distinct tags, which bounds the symbolic domain.
    const TAG_COUNT: u8 = 9;

    /// Pick the token tag for a bounded selector value.
    fn tag_for(value: u8) -> TokenTag {
        kani::assume(value < TAG_COUNT);
        match value {
            0 => TokenTag::Install,
            1 => TokenTag::Ls,
            2 => TokenTag::Check,
            3 => TokenTag::Help,
            4 => TokenTag::Version,
            5 => TokenTag::DoubleDash,
            6 => TokenTag::GlobalFlag,
            7 => TokenTag::DylintFlag,
            _ => TokenTag::Other,
        }
    }

    /// Classify tokens the way the router did before `check` became a command.
    ///
    /// This is the regression oracle for `CLI-REQ-FWD`: every argument vector
    /// that meant something before must still mean exactly the same thing.
    /// Its one intentional exception is a vector whose first significant
    /// token is [`TokenTag::Check`]: the oracle forwards it because there was
    /// no `check` command to select.
    fn classify_before_check(tokens: &[TokenTag]) -> Class {
        for token in tokens {
            match token {
                TokenTag::GlobalFlag => {}
                TokenTag::Install => return Class::Subcommand(Subcommand::Install),
                TokenTag::Ls => return Class::Subcommand(Subcommand::Ls),
                TokenTag::Help | TokenTag::Version => return Class::Display,
                TokenTag::Check | TokenTag::DoubleDash | TokenTag::DylintFlag | TokenTag::Other => {
                    return Class::Forward;
                }
            }
        }
        Class::Forward
    }

    /// Return the first token that is not a root-level global flag.
    fn first_significant(tokens: &[TokenTag]) -> Option<TokenTag> {
        tokens
            .iter()
            .copied()
            .find(|token| *token != TokenTag::GlobalFlag)
    }

    /// Check that the result follows the first significant token.
    #[kani::proof]
    #[kani::unwind(7)]
    fn verify_classify_is_total_and_disjoint() {
        let raw: [u8; 6] = kani::any();
        let length: usize = kani::any();
        kani::assume(length <= raw.len());
        let tags = raw.map(tag_for);
        let bounded = tags.get(..length).unwrap_or_default();
        let result = classify(bounded);
        match first_significant(bounded) {
            Some(TokenTag::Install) => assert!(
                result == Class::Subcommand(Subcommand::Install),
                "an install token must select the install subcommand"
            ),
            Some(TokenTag::Ls) => assert!(
                result == Class::Subcommand(Subcommand::Ls),
                "an ls token must select the ls subcommand"
            ),
            Some(TokenTag::Check) => assert!(
                result == Class::Subcommand(Subcommand::Check),
                "a first significant check token must select the check subcommand"
            ),
            Some(TokenTag::Help | TokenTag::Version) => assert!(
                result == Class::Display,
                "a help or version token must display"
            ),
            Some(TokenTag::DoubleDash | TokenTag::DylintFlag | TokenTag::Other) | None => {
                assert!(
                    result == Class::Forward,
                    "a forwarding token, or no token at all, must forward"
                );
            }
            Some(TokenTag::GlobalFlag) => assert!(false, "filter must skip global flags"),
        }
    }

    /// Check that vectors not led by `Check` route exactly as the oracle did.
    #[kani::proof]
    #[kani::unwind(7)]
    fn verify_check_does_not_change_legacy_routing() {
        let raw: [u8; 6] = kani::any();
        let length: usize = kani::any();
        kani::assume(length <= raw.len());
        let tags = raw.map(tag_for);
        let bounded = tags.get(..length).unwrap_or_default();
        kani::assume(!matches!(first_significant(bounded), Some(TokenTag::Check)));

        assert!(
            classify(bounded) == classify_before_check(bounded),
            "a vector not led by a check token must route exactly as it used to"
        );
    }

    /// Witness the check route before failing deliberately.
    #[kani::proof]
    fn verify_check_is_reachable() {
        assert!(
            classify(&[TokenTag::Check]) == Class::Subcommand(Subcommand::Check),
            "a check token must select the check subcommand"
        );
        assert!(false, "the check subcommand should have a counterexample");
    }

    /// Witness the subcommand route before failing deliberately.
    #[kani::proof]
    fn verify_subcommand_is_reachable() {
        assert!(
            classify(&[TokenTag::Install]) == Class::Subcommand(Subcommand::Install),
            "an install token must select the install subcommand"
        );
        assert!(false, "subcommand class should have a counterexample");
    }

    /// Witness the forwarding route before failing deliberately.
    #[kani::proof]
    fn verify_forward_is_reachable() {
        assert!(
            classify(&[TokenTag::DylintFlag]) == Class::Forward,
            "a dylint flag must forward"
        );
        assert!(false, "forward class should have a counterexample");
    }

    /// Witness the display route before failing deliberately.
    #[kani::proof]
    fn verify_display_is_reachable() {
        assert!(
            classify(&[TokenTag::Help]) == Class::Display,
            "a help request must display"
        );
        assert!(false, "display class should have a counterexample");
    }
}
