//! Which packages Cargo will check, for the `-v` coverage listing.
//!
//! The rules are Cargo's, not Whitaker's: `--workspace` (and its deprecated
//! `--all` alias) selects every member, `-p` selects the named members (with
//! glob patterns), `--exclude` removes members in workspace mode, and no
//! selection flag at all selects what Cargo reports as the default members for
//! the current context. That last answer is read from `cargo metadata` rather
//! than recomputed here, so it cannot drift from what `cargo clippy` does.
//!
//! This module is pure. It never runs Cargo and never writes output; it turns
//! workspace facts and parsed options into the ordered package list the command
//! layer prints.

use std::collections::BTreeSet;

use camino::Utf8Path;
use thiserror::Error;

use super::cargo::CargoOptions;
use crate::ports::{PackageFacts, WorkspaceFacts};

/// Why a selection could not be resolved.
///
/// Cargo treats both cases as errors of its own, so the listing is skipped and
/// the run still proceeds: the child reports the authoritative diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SelectionError {
    /// A `-p` selector matched no workspace member.
    #[error("package selector `{spec}` matched no workspace member")]
    UnknownPackage {
        /// The selector as the caller wrote it.
        spec: String,
    },
    /// An `--exclude` selector matched no workspace member.
    #[error("exclude selector `{spec}` matched no workspace member")]
    UnknownExclude {
        /// The selector as the caller wrote it.
        spec: String,
    },
    /// `--exclude` was given without a workspace selector.
    #[error("`--exclude` requires `--workspace` or `--all`")]
    ExcludeWithoutWorkspace,
}

/// Resolve the packages the options select.
///
/// The order is the workspace's member order, so the listing is deterministic
/// without inventing an ordering of its own. `--workspace` wins over `-p`, as
/// it does in Cargo, and `--exclude` only applies in workspace mode.
///
/// # Examples
///
/// ```
/// use whitaker::{
///     cli::check::{CargoOptions, selection::selected_packages},
///     ports::{PackageFacts, WorkspaceFacts},
/// };
///
/// let facts = WorkspaceFacts {
///     workspace_root: "/work/app".into(),
///     members: vec![PackageFacts {
///         name: "app".to_owned(),
///         version: "0.1.0".to_owned(),
///         manifest_path: "/work/app/Cargo.toml".into(),
///     }],
///     default_members: Vec::new(),
/// };
/// let selected = selected_packages(&facts, &CargoOptions::default())?;
/// assert_eq!(selected.len(), 1, "the default selection must be shown");
/// # Ok::<(), whitaker::cli::check::selection::SelectionError>(())
/// ```
///
/// # Errors
///
/// Returns [`SelectionError`] when a selector names nothing in the workspace.
pub fn selected_packages<'a>(
    facts: &'a WorkspaceFacts,
    options: &CargoOptions,
) -> Result<Vec<&'a PackageFacts>, SelectionError> {
    let is_workspace = options.workspace || options.all;
    if !is_workspace && !options.excludes.is_empty() {
        return Err(SelectionError::ExcludeWithoutWorkspace);
    }
    if is_workspace {
        return without_excluded(&facts.members, &options.excludes);
    }
    if !options.packages.is_empty() {
        return named_members(&facts.members, &options.packages);
    }
    Ok(facts.default_members.iter().collect())
}

/// Render one Cargo-style `Checking` line for a package.
///
/// The format is Cargo's own, so a reader compares like with like: four spaces,
/// `Checking`, the name, `v` and the version, then the package directory in
/// parentheses.
///
/// # Examples
///
/// ```
/// use whitaker::{cli::check::selection::checking_line, ports::PackageFacts};
///
/// let package = PackageFacts {
///     name: "app".to_owned(),
///     version: "0.1.0".to_owned(),
///     manifest_path: "/work/app/Cargo.toml".into(),
/// };
/// assert_eq!(
///     checking_line(&package),
///     "    Checking app v0.1.0 (/work/app)",
/// );
/// ```
#[must_use]
pub fn checking_line(package: &PackageFacts) -> String {
    format!(
        "    Checking {} v{} ({})",
        package.name,
        package.version,
        package.directory()
    )
}

/// Keep every member the excludes do not name.
fn without_excluded<'a>(
    members: &'a [PackageFacts],
    excludes: &[String],
) -> Result<Vec<&'a PackageFacts>, SelectionError> {
    for spec in excludes {
        if !members.iter().any(|package| spec_matches(spec, package)) {
            return Err(SelectionError::UnknownExclude { spec: spec.clone() });
        }
    }
    Ok(members
        .iter()
        .filter(|package| !excludes.iter().any(|spec| spec_matches(spec, package)))
        .collect())
}

/// Keep the members each selector names, in member order and without repeats.
///
/// A selector that names nothing is an error even when another selector
/// matched, because Cargo rejects the whole invocation in that case; reporting
/// it here keeps the listing from implying a run that Cargo would refuse.
fn named_members<'a>(
    members: &'a [PackageFacts],
    selectors: &[String],
) -> Result<Vec<&'a PackageFacts>, SelectionError> {
    let mut named: BTreeSet<&Utf8Path> = BTreeSet::new();
    for spec in selectors {
        let mut matched = false;
        for package in members {
            if spec_matches(spec, package) {
                matched = true;
                named.insert(package.manifest_path.as_path());
            }
        }
        if !matched {
            return Err(SelectionError::UnknownPackage { spec: spec.clone() });
        }
    }
    Ok(members
        .iter()
        .filter(|package| named.contains(package.manifest_path.as_path()))
        .collect())
}

/// Report whether a selector names a package.
///
/// Cargo accepts a package name, a `name@version` pair, or a pattern using `*`
/// and `?`. The version is matched as written, which covers the `name@1.2.3`
/// form; a partial requirement such as `name@1` is not expanded here, because
/// the listing is advisory and Cargo still performs the real match.
fn spec_matches(spec: &str, package: &PackageFacts) -> bool {
    if is_pattern(spec) {
        return glob_matches(spec, &package.name);
    }
    match spec.split_once('@') {
        Some((name, version)) => name == package.name && version == package.version,
        None => spec == package.name,
    }
}

/// Report whether a selector uses Cargo's pattern syntax.
fn is_pattern(spec: &str) -> bool { spec.contains(['*', '?']) }

/// Match a name against a `*`/`?` pattern.
///
/// Package names are short and patterns come from a command line, so the plain
/// recursive definition is clearer than a memoized matcher and costs nothing
/// measurable.
fn glob_matches(pattern: &str, name: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
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
    //! Selection rules over hand-built workspace facts.

    use googletest::prelude::*;
    use rstest::rstest;

    use super::{SelectionError, checking_line, glob_matches, selected_packages};
    use crate::{
        cli::check::CargoOptions,
        ports::{PackageFacts, WorkspaceFacts},
    };

    /// Build one package with a manifest under `root`.
    fn package(name: &str, version: &str, root: &str) -> PackageFacts {
        PackageFacts {
            name: name.to_owned(),
            version: version.to_owned(),
            manifest_path: format!("{root}/{name}/Cargo.toml").into(),
        }
    }

    /// Three members whose default selection is the middle one.
    fn facts() -> WorkspaceFacts {
        let member_a = package("member_a", "0.1.0", "/work/app");
        let member_b = package("member_b", "0.1.0", "/work/app");
        let rootpkg = package("rootpkg", "0.2.0", "/work/app");
        WorkspaceFacts {
            workspace_root: "/work/app".into(),
            members: vec![rootpkg.clone(), member_a, member_b.clone()],
            default_members: vec![member_b],
        }
    }

    /// Read the selected names for a set of options.
    fn names(options: &CargoOptions) -> Vec<String> {
        selected_packages(&facts(), options)
            .expect("the selection resolves")
            .into_iter()
            .map(|package| package.name.clone())
            .collect()
    }

    /// Select each option shape and compare with the expected names.
    #[rstest]
    #[case::default(CargoOptions::default(), vec!["member_b"])]
    #[case::workspace(
        CargoOptions { workspace: true, ..CargoOptions::default() },
        vec!["rootpkg", "member_a", "member_b"]
    )]
    #[case::deprecated_all(
        CargoOptions { all: true, ..CargoOptions::default() },
        vec!["rootpkg", "member_a", "member_b"]
    )]
    #[case::workspace_wins_over_packages(
        CargoOptions {
            workspace: true,
            packages: vec!["member_a".to_owned()],
            ..CargoOptions::default()
        },
        vec!["rootpkg", "member_a", "member_b"]
    )]
    #[case::workspace_with_exclude(
        CargoOptions {
            workspace: true,
            excludes: vec!["member_a".to_owned()],
            ..CargoOptions::default()
        },
        vec!["rootpkg", "member_b"]
    )]
    #[case::exclude_pattern(
        CargoOptions {
            workspace: true,
            excludes: vec!["member_*".to_owned()],
            ..CargoOptions::default()
        },
        vec!["rootpkg"]
    )]
    #[case::one_package(
        CargoOptions {
            packages: vec!["member_a".to_owned()],
            ..CargoOptions::default()
        },
        vec!["member_a"]
    )]
    #[case::package_pattern(
        CargoOptions {
            packages: vec!["member_*".to_owned()],
            ..CargoOptions::default()
        },
        vec!["member_a", "member_b"]
    )]
    #[case::repeated_package(
        CargoOptions {
            packages: vec!["member_b".to_owned(), "member_b".to_owned()],
            ..CargoOptions::default()
        },
        vec!["member_b"]
    )]
    fn selection_follows_cargos_rules(#[case] options: CargoOptions, #[case] expected: Vec<&str>) {
        assert_eq!(
            names(&options),
            expected,
            "the selected set must follow Cargo's rules"
        );
    }

    /// Refuse a selector that names nothing.
    #[rstest]
    #[case::package(
        CargoOptions { packages: vec!["absent".to_owned()], ..CargoOptions::default() },
        SelectionError::UnknownPackage { spec: "absent".to_owned() }
    )]
    #[case::exclude(
        CargoOptions {
            workspace: true,
            excludes: vec!["absent".to_owned()],
            ..CargoOptions::default()
        },
        SelectionError::UnknownExclude { spec: "absent".to_owned() }
    )]
    #[case::exclude_without_workspace(
        CargoOptions {
            excludes: vec!["member_a".to_owned()],
            ..CargoOptions::default()
        },
        SelectionError::ExcludeWithoutWorkspace
    )]
    fn unknown_selectors_are_reported(
        #[case] options: CargoOptions,
        #[case] expected: SelectionError,
    ) {
        let error = selected_packages(&facts(), &options)
            .expect_err("a selector naming nothing must be reported");

        assert_eq!(error, expected, "the error must name the selector");
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
            glob_matches(pattern, name),
            expected,
            "pattern {pattern:?} against {name:?}"
        );
    }

    /// Render Cargo's own progress-line shape.
    #[test]
    fn the_checking_line_names_the_package_and_its_directory() -> googletest::Result<()> {
        let package = package("member_a", "0.1.0", "/work/app");

        verify_that!(
            checking_line(&package),
            eq("    Checking member_a v0.1.0 (/work/app/member_a)")
        )
    }
}
