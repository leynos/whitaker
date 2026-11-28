//! Cargo's glob pattern matching for package selectors.
//!
//! Cargo accepts glob patterns wherever a package selector is allowed, with
//! the syntax of the `glob` crate: `*`, `?`, and bracket classes such as
//! `[ab]`, `[a-z]` and `[!a]`. The selection listing has to resolve them itself
//! to show the packages a run will cover, but it never has to be more precise
//! than Cargo: a selector this matcher cannot resolve leaves the listing
//! unavailable, while Cargo still performs the authoritative match for the run.
//!
//! The matcher parses the pattern once and walks it with the classic two-pointer
//! algorithm, so an adversarial pattern of many `*a` pairs cannot cause the
//! exponential backtracking a plain recursive matcher would. A `[` without a
//! closing `]` is a literal, which is what Cargo's matcher does.

use crate::ports::{PackageFacts, PackageSpec};

/// One parsed pattern element.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// A literal character.
    Literal(char),
    /// `?`: exactly one character.
    Any,
    /// `*`: any run of characters, including none.
    Star,
    /// `[...]`: one character from the listed ranges.
    Class {
        /// Whether the class matches characters *not* in its ranges.
        negated: bool,
        /// Inclusive character ranges; a single character is `(c, c)`.
        ranges: Vec<(char, char)>,
    },
}

impl Token {
    /// Report whether one name character matches this token.
    fn matches(&self, character: char) -> bool {
        match self {
            Self::Literal(literal) => *literal == character,
            Self::Any => true,
            Self::Star => false,
            Self::Class { negated, ranges } => {
                let listed = ranges
                    .iter()
                    .any(|(low, high)| low <= &character && &character <= high);
                listed != *negated
            }
        }
    }
}

/// Report whether a selector uses Cargo's pattern syntax.
pub fn is_pattern(spec: &PackageSpec) -> bool { spec.as_str().contains(['*', '?', '[']) }

/// Match a glob pattern against a bare name, such as a target name.
///
/// Cargo accepts the same patterns for `--bin`, `--test`, `--example` and
/// `--bench` names as it does for package selectors. An invalid pattern
/// matches nothing, which leaves the listing unavailable rather than guessing.
#[must_use]
pub fn matches_name(pattern: &str, name: &str) -> bool {
    let Some(tokens) = parse(pattern) else {
        return false;
    };
    let characters: Vec<char> = name.chars().collect();
    matches_from(&tokens, &characters)
}

/// Match a package against a glob selector.
///
/// Package names are short and patterns come from a command line, so parsing
/// once per comparison is clearer than caching, and the walk itself is
/// polynomial in the product of the two lengths.
pub fn matches(pattern: &PackageSpec, package: &PackageFacts) -> bool {
    matches_name(pattern.as_str(), &package.name)
}

/// Parse a pattern into tokens, or report a malformed bracket class.
///
/// Cargo's `glob` matcher rejects an unclosed or empty class, and Cargo rejects
/// the selector with it, so the listing treats the pattern as unresolved rather
/// than as a literal.
fn parse(pattern: &str) -> Option<Vec<Token>> {
    let characters: Vec<char> = pattern.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while let Some(&character) = characters.get(index) {
        match character {
            '*' => {
                tokens.push(Token::Star);
                index += 1;
            }
            '?' => {
                tokens.push(Token::Any);
                index += 1;
            }
            '[' => {
                let (token, next) = parse_class(&characters, index + 1)?;
                tokens.push(token);
                index = next;
            }
            literal => {
                tokens.push(Token::Literal(literal));
                index += 1;
            }
        }
    }
    Some(tokens)
}

/// Parse a bracket class starting after its `[`.
///
/// Only `!` negates; `^` is an ordinary member, as it is in Cargo's matcher.
/// Returns the token and the index after the closing `]`, or `None` when the
/// class is unclosed or empty. A `]` immediately after the opening bracket (or
/// after the negation marker) is a literal member of the class, and a `-` at
/// the end of the class is literal too.
fn parse_class(characters: &[char], start: usize) -> Option<(Token, usize)> {
    let (negated, mut index) = negation_marker(characters, start);
    let mut ranges = Vec::new();
    let mut first = true;
    while let Some(&character) = characters.get(index) {
        if character == ']' {
            // An empty class (`[]`) is malformed in Cargo's matcher, while a
            // closing bracket after a member ends the class.
            return if first {
                None
            } else {
                Some((Token::Class { negated, ranges }, index + 1))
            };
        }
        first = false;
        let (range, next) = class_member(characters, index);
        ranges.push(range);
        index = next;
    }
    None
}

/// Read an optional negation marker after the opening bracket.
fn negation_marker(characters: &[char], index: usize) -> (bool, usize) {
    match characters.get(index) {
        Some(&'!') => (true, index + 1),
        _ => (false, index),
    }
}

/// Parse one class member, `a` or `a-z`, starting at `index`.
///
/// A `-` immediately before the closing bracket is a literal member, as it is
/// in Cargo's matcher.
fn class_member(characters: &[char], index: usize) -> ((char, char), usize) {
    let character = characters.get(index).copied().unwrap_or(']');
    match (characters.get(index + 1), characters.get(index + 2)) {
        (Some(&'-'), Some(&high)) if high != ']' => ((character, high), index + 3),
        _ => ((character, character), index + 1),
    }
}

/// Match the tokens against the name without exponential backtracking.
///
/// The engine remembers the last `*` and the name position it resumed from, so
/// the worst case is proportional to the product of the two lengths. Each
/// resume advances the name, and the pattern is never re-parsed.
fn matches_from(tokens: &[Token], name: &[char]) -> bool {
    let mut token_index = 0;
    let mut name_index = 0;
    let mut star: Option<usize> = None;
    let mut resume = 0;

    while let Some(&character) = name.get(name_index) {
        match tokens.get(token_index) {
            Some(Token::Star) => {
                star = Some(token_index);
                token_index += 1;
                resume = name_index;
            }
            Some(token) if token.matches(character) => {
                token_index += 1;
                name_index += 1;
            }
            _ => {
                let Some(star_index) = star else {
                    return false;
                };
                token_index = star_index + 1;
                resume += 1;
                name_index = resume;
            }
        }
    }

    while tokens.get(token_index) == Some(&Token::Star) {
        token_index += 1;
    }
    token_index == tokens.len()
}

#[cfg(test)]
mod tests {
    //! The pattern shapes Cargo accepts for package names.

    use rstest::rstest;

    use super::{is_pattern, matches};
    use crate::ports::{PackageFacts, PackageSpec};

    /// Build one package with a manifest under `/work/app`.
    fn package(name: &str) -> PackageFacts {
        PackageFacts {
            name: name.to_owned(),
            version: "0.1.0".to_owned(),
            manifest_path: format!("/work/app/{name}/Cargo.toml").into(),
            targets: Vec::new(),
        }
    }

    /// Match the pattern shapes Cargo accepts for package names.
    #[rstest]
    #[case::exact("member_a", "member_a", true)]
    #[case::prefix("member_*", "member_a", true)]
    #[case::suffix("*_a", "member_a", true)]
    #[case::single("member_?", "member_a", true)]
    #[case::one_character("member_??", "member_a", false)]
    #[case::too_long("member_?", "member_ab", false)]
    #[case::any("m?mber_*", "member_a", true)]
    #[case::empty_pattern("", "member_a", false)]
    #[case::only_star("*", "member_a", true)]
    #[case::class("member_[ab]", "member_a", true)]
    #[case::class_rejects("member_[bc]", "member_a", false)]
    #[case::class_range("member_[a-c]", "member_b", true)]
    #[case::class_negated("member_[!b]", "member_a", true)]
    #[case::class_negated_rejects("member_[!a]", "member_a", false)]
    #[case::class_with_star("alpha-[on]*", "alpha-one", true)]
    #[case::class_with_star_rejects("alpha-[py]*", "alpha-one", false)]
    #[case::unclosed_bracket_is_unresolved("member_[a", "member_[a", false)]
    #[case::unclosed_bracket_is_not_a_class("member_[a", "member_a", false)]
    #[case::caret_is_a_literal_member("[^a]lpha-*", "alpha-one", true)]
    #[case::caret_is_not_negation("[^a]lpha-*", "beta", false)]
    #[case::trailing_dash_is_literal("member_[a-]", "member_-", true)]
    fn glob_matching_covers_the_documented_syntax(
        #[case] pattern: &str,
        #[case] name: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(
            matches(&PackageSpec::from(pattern), &package(name)),
            expected,
            "pattern {pattern:?} against {name:?}"
        );
    }

    /// Reject an adversarial pattern without exponential backtracking.
    ///
    /// A recursive definition revisits positions exponentially often for this
    /// shape; the test finishes only because the matcher keeps the last `*`
    /// position instead. Ten `*a` pairs against forty `a` characters would be
    /// over a million visits under backtracking.
    #[test]
    fn an_adversarial_pattern_is_rejected_quickly() {
        let pattern = PackageSpec::from("*a*a*a*a*a*a*a*a*a*a*b");
        let name = "a".repeat(40);

        assert!(
            !matches(&pattern, &package(&name)),
            "the trailing b is absent, so the pattern must not match"
        );
    }

    /// Classify selectors by whether Cargo treats them as patterns.
    #[rstest]
    #[case::exact("member_a", false)]
    #[case::star("member_*", true)]
    #[case::question("member_?", true)]
    #[case::class("member_[ab]", true)]
    #[case::version("member_a@0.1.0", false)]
    fn pattern_detection_covers_the_syntax(#[case] spec: &str, #[case] expected: bool) {
        assert_eq!(
            is_pattern(&PackageSpec::from(spec)),
            expected,
            "selector {spec:?}"
        );
    }
}
