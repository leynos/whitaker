//! Which packages Cargo will check, for the `-v` coverage listing.
//!
//! The rules are Cargo's, not Whitaker's: `--workspace` (and its deprecated
//! `--all` alias) selects every member, `-p` selects the named members,
//! `--exclude` removes members in workspace mode, and no selection flag at all
//! selects what Cargo reports as the default members for the current context.
//! That last answer is read from `cargo metadata` rather than recomputed here,
//! so it cannot drift from what `cargo clippy` does.
//!
//! This module is pure. It never runs Cargo and never writes output; it turns
//! workspace facts and parsed options into the ordered package list the command
//! layer prints. Selector matching lives in [`spec`] and the target-eligibility
//! rules in [`targets`].

use std::collections::BTreeSet;

use camino::Utf8Path;
use thiserror::Error;

use super::{cargo::CargoOptions, spec, targets};
use crate::ports::{PackageFacts, PackageSpec, WorkspaceFacts};

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
        spec: PackageSpec,
    },
    /// `--exclude` was given without a workspace selector.
    #[error("`--exclude` requires `--workspace` or `--all`")]
    ExcludeWithoutWorkspace,
    /// A selector's form could not be resolved.
    ///
    /// Cargo may still understand it, so the listing is suppressed rather than
    /// treating an unreadable selector as a confirmed non-match.
    #[error("selector `{spec}` is not in a form this listing can resolve")]
    UnresolvedSelector {
        /// The selector as the caller wrote it.
        spec: PackageSpec,
    },
}

/// Resolve the packages the options select.
///
/// The order is the workspace's member order, so the listing is deterministic
/// without inventing an ordering of its own. `--workspace` wins over `-p`, as
/// it does in Cargo, and `--exclude` only applies in workspace mode. A selected
/// package whose targets the target selectors do not reach is dropped, because
/// Cargo will not check it either.
///
/// # Examples
///
/// ```
/// use whitaker::{
///     cli::check::{CargoOptions, selection::selected_packages},
///     ports::{PackageFacts, TargetFacts, WorkspaceFacts},
/// };
///
/// let package = PackageFacts {
///     name: "app".to_owned(),
///     version: "0.1.0".to_owned(),
///     manifest_path: "/work/app/Cargo.toml".into(),
///     targets: vec![TargetFacts {
///         name: "app".to_owned(),
///         kinds: vec!["lib".to_owned()],
///         required_features: Vec::new(),
///         has_test_harness: true,
///     }],
///     features: Default::default(),
/// };
/// let facts = WorkspaceFacts {
///     workspace_root: "/work/app".into(),
///     members: vec![package.clone()],
///     default_members: vec![package],
/// };
/// let selected = selected_packages(&facts, &CargoOptions::default())?;
/// assert_eq!(selected.len(), 1, "the default selection must be shown");
/// # Ok::<(), whitaker::cli::check::selection::SelectionError>(())
/// ```
///
/// # Errors
///
/// Returns [`SelectionError`] when a `-p` selector names nothing in the
/// workspace, or when `--exclude` is given without a workspace selector.
pub fn selected_packages<'a>(
    facts: &'a WorkspaceFacts,
    options: &CargoOptions,
) -> Result<Vec<&'a PackageFacts>, SelectionError> {
    let is_workspace = options.workspace || options.all;
    if !is_workspace && !options.excludes.is_empty() {
        return Err(SelectionError::ExcludeWithoutWorkspace);
    }
    // Cargo validates every supplied `-p` selector even when `--workspace`
    // makes the selection wider, and rejects the invocation when one names
    // nothing; resolving them first keeps the listing from implying a run that
    // Cargo would refuse.
    let named = if options.packages.is_empty() {
        None
    } else {
        let selectors = specs(&options.packages);
        Some(named_members(&facts.members, &selectors)?)
    };
    let candidates: Vec<&PackageFacts> = if is_workspace {
        let excludes = specs(&options.excludes);
        without_excluded(&facts.members, &excludes)?
    } else if let Some(selected) = named {
        selected
    } else {
        facts.default_members.iter().collect()
    };
    Ok(candidates
        .into_iter()
        .filter(|package| targets::is_checked(package, options))
        .collect())
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
///     targets: Vec::new(),
///     features: Default::default(),
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

/// Wrap each selector the caller gave.
///
/// The parsed options keep Cargo's spelling as plain strings because that is
/// what clap collects; the selection logic works in the package-selector type
/// so its arguments express what they mean.
fn specs(selectors: &[String]) -> Vec<PackageSpec> {
    selectors.iter().cloned().map(PackageSpec::from).collect()
}

/// Keep every member the excludes do not name.
///
/// An exclusion that names nothing is not an error: Cargo warns and continues
/// with the remaining set, and the listing follows the same rule. An exclusion
/// whose form cannot be resolved is different: Cargo may understand it, so the
/// listing is suppressed rather than silently keeping the member.
fn without_excluded<'a>(
    members: &'a [PackageFacts],
    excludes: &[PackageSpec],
) -> Result<Vec<&'a PackageFacts>, SelectionError> {
    let mut kept = Vec::new();
    for package in members {
        let mut is_excluded = false;
        for spec in excludes {
            match spec::matches(spec, package) {
                Some(true) => {
                    is_excluded = true;
                    break;
                }
                Some(false) => {}
                None => {
                    return Err(SelectionError::UnresolvedSelector { spec: spec.clone() });
                }
            }
        }
        if !is_excluded {
            kept.push(package);
        }
    }
    Ok(kept)
}

/// Keep the members each selector names, in member order and without repeats.
///
/// A selector that names nothing is an error even when another selector
/// matched, because Cargo rejects the whole invocation in that case; reporting
/// it here keeps the listing from implying a run that Cargo would refuse.
fn named_members<'a>(
    members: &'a [PackageFacts],
    selectors: &[PackageSpec],
) -> Result<Vec<&'a PackageFacts>, SelectionError> {
    let mut named: BTreeSet<&Utf8Path> = BTreeSet::new();
    for spec in selectors {
        let mut matched = false;
        for package in members {
            match spec::matches(spec, package) {
                Some(true) => {
                    matched = true;
                    named.insert(package.manifest_path.as_path());
                }
                Some(false) => {}
                None => {
                    return Err(SelectionError::UnresolvedSelector { spec: spec.clone() });
                }
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

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
