//! Cargo's `*` and `?` pattern matching for package selectors.
//!
//! Cargo accepts glob patterns wherever a package selector is allowed. The
//! selection listing has to resolve them itself to show the packages a run will
//! cover, but it never has to be more precise than Cargo: a selector this
//! matcher cannot resolve is reported and the listing is skipped, while Cargo
//! still performs the authoritative match for the run.

use crate::ports::{PackageFacts, PackageSpec};

/// Report whether a selector uses Cargo's pattern syntax.
pub(super) fn is_pattern(spec: &PackageSpec) -> bool { spec.as_str().contains(['*', '?']) }

/// Match a package against a `*`/`?` selector.
///
/// Package names are short and patterns come from a command line, so the plain
/// recursive definition is clearer than a memoized matcher and costs nothing
/// measurable.
pub(super) fn matches(pattern: &PackageSpec, package: &PackageFacts) -> bool {
    let pattern_chars: Vec<char> = pattern.as_str().chars().collect();
    let name_chars: Vec<char> = package.name.chars().collect();
    matches_from(&pattern_chars, &name_chars)
}

/// Match the pattern tail against the name tail.
fn matches_from(pattern: &[char], name: &[char]) -> bool {
    match (pattern.split_first(), name.split_first()) {
        (None, None) => true,
        (Some((&'*', rest)), _) => {
            matches_from(rest, name)
                || name
                    .split_first()
                    .is_some_and(|(_, tail)| matches_from(pattern, tail))
        }
        (Some((&'?', rest)), Some((_, tail))) => matches_from(rest, tail),
        (Some((&literal, rest)), Some((&character, tail))) => {
            literal == character && matches_from(rest, tail)
        }
        (None, Some(_)) | (Some(_), None) => false,
    }
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

    /// Classify selectors by whether Cargo treats them as patterns.
    #[rstest]
    #[case::exact("member_a", false)]
    #[case::star("member_*", true)]
    #[case::question("member_?", true)]
    #[case::version("member_a@0.1.0", false)]
    fn pattern_detection_covers_the_syntax(#[case] spec: &str, #[case] expected: bool) {
        assert_eq!(
            is_pattern(&PackageSpec::from(spec)),
            expected,
            "selector {spec:?}"
        );
    }
}
