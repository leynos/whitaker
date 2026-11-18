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
/// );
/// assert_eq!(classify(&[TokenTag::DylintFlag]), Class::Forward);
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

    /// The classifier as it stood before `check` became a command.
    ///
    /// This is the regression oracle for `CLI-REQ-FWD`: every argument vector
    /// that meant something before must still mean exactly the same thing.
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

    fn first_significant(tokens: &[TokenTag]) -> Option<TokenTag> {
        tokens
            .iter()
            .copied()
            .find(|token| *token != TokenTag::GlobalFlag)
    }

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
            Some(TokenTag::Install) => {
                assert!(result == Class::Subcommand(Subcommand::Install));
            }
            Some(TokenTag::Ls) => assert!(result == Class::Subcommand(Subcommand::Ls)),
            Some(TokenTag::Check) => assert!(result == Class::Subcommand(Subcommand::Check)),
            Some(TokenTag::Help | TokenTag::Version) => assert!(result == Class::Display),
            Some(TokenTag::DoubleDash | TokenTag::DylintFlag | TokenTag::Other) | None => {
                assert!(result == Class::Forward);
            }
            Some(TokenTag::GlobalFlag) => assert!(false, "filter must skip global flags"),
        }
    }

    #[kani::proof]
    #[kani::unwind(7)]
    fn verify_check_does_not_change_legacy_routing() {
        let raw: [u8; 6] = kani::any();
        let length: usize = kani::any();
        kani::assume(length <= raw.len());
        let tags = raw.map(tag_for);
        let bounded = tags.get(..length).unwrap_or_default();
        kani::assume(!bounded.contains(&TokenTag::Check));

        assert!(
            classify(bounded) == classify_before_check(bounded),
            "a vector with no check token must route exactly as it used to"
        );
    }

    #[kani::proof]
    fn verify_check_is_reachable() {
        assert!(classify(&[TokenTag::Check]) == Class::Subcommand(Subcommand::Check));
        assert!(false, "the check subcommand should have a counterexample");
    }

    #[kani::proof]
    fn verify_subcommand_is_reachable() {
        assert!(classify(&[TokenTag::Install]) == Class::Subcommand(Subcommand::Install));
        assert!(false, "subcommand class should have a counterexample");
    }

    #[kani::proof]
    fn verify_forward_is_reachable() {
        assert!(classify(&[TokenTag::DylintFlag]) == Class::Forward);
        assert!(false, "forward class should have a counterexample");
    }

    #[kani::proof]
    fn verify_display_is_reachable() {
        assert!(classify(&[TokenTag::Help]) == Class::Display);
        assert!(false, "display class should have a counterexample");
    }
}
