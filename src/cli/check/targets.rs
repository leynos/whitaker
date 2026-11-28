//! Target eligibility for the coverage listing.
//!
//! Cargo selects packages, then builds only the targets the target selectors
//! reach. A workspace member whose only target is a library contributes
//! nothing to `--bins`, so the listing must not claim Cargo will check it. The
//! rules below mirror Cargo's documented selection: the default is the library
//! and binary targets, `--all-targets` is everything, and every other selector
//! contributes the targets it names.
//!
//! A package whose facts carry no target information is kept: the listing
//! exists to expose coverage, and hiding a package on missing data would be the
//! worse error. Cargo remains the authority for the run itself.

use std::collections::BTreeSet;

use super::{cargo::CargoOptions, glob};
use crate::ports::{PackageFacts, TargetFacts};

/// Report whether the target selectors leave work for `package`.
///
/// A target whose `required_features` are not enabled is skipped by Cargo, so
/// it is skipped here too. Eligibility is decided over the enabled targets
/// only; the aggregate `--tests` and `--benches` selectors follow Cargo's
/// per-target harness flags rather than the target kind.
///
/// # Examples
///
/// ```
/// use whitaker::{
///     cli::check::{CargoOptions, targets::is_checked},
///     ports::{PackageFacts, TargetFacts},
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
/// assert!(is_checked(&package, &CargoOptions::default()));
/// assert!(!is_checked(
///     &package,
///     &CargoOptions {
///         bins: true,
///         ..CargoOptions::default()
///     },
/// ));
/// ```
#[must_use]
pub fn is_checked(package: &PackageFacts, options: &CargoOptions) -> bool {
    if package.targets.is_empty() {
        return true;
    }
    let enabled: Vec<&TargetFacts> = package
        .targets
        .iter()
        .filter(|target| features_enabled(target, package, options))
        .collect();
    if options.all_targets {
        return !enabled.is_empty();
    }
    if any_selector(options) {
        return selector_matches(&enabled, options);
    }
    default_selected(&enabled)
}

/// Report whether the default selection (library and binary targets) has work.
fn default_selected(targets: &[&TargetFacts]) -> bool {
    targets
        .iter()
        .any(|target| target.is_library() || target.is_binary())
}

/// Report whether any explicit selector reaches the enabled targets.
fn selector_matches(targets: &[&TargetFacts], options: &CargoOptions) -> bool {
    library_selected(targets, options)
        || binaries_selected(targets, options)
        || tests_selected(targets, options)
        || examples_selected(targets, options)
        || benches_selected(targets, options)
}

/// Report whether the caller's feature selection enables `target`.
fn features_enabled(target: &TargetFacts, package: &PackageFacts, options: &CargoOptions) -> bool {
    if target.required_features.is_empty() {
        return true;
    }
    let enabled = enabled_features(package, options);
    target
        .required_features
        .iter()
        .all(|required| enabled.contains(required))
}

/// Resolve the features this invocation enables for `package`.
///
/// `--all-features` enables every declared feature. Otherwise the `default`
/// feature is expanded unless `--no-default-features` was given, the features
/// the caller names are added, and every implied feature is followed.
fn enabled_features(package: &PackageFacts, options: &CargoOptions) -> BTreeSet<String> {
    if options.all_features {
        return package.features.keys().cloned().collect();
    }
    let mut pending: Vec<String> = Vec::new();
    if !options.no_default_features && package.features.contains_key("default") {
        pending.push("default".to_owned());
    }
    pending.extend(
        options
            .features
            .iter()
            .flat_map(|list| list.split([',', ' ']))
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
    );
    let mut enabled = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !enabled.insert(name.clone()) {
            continue;
        }
        if let Some(implied) = package.features.get(&name) {
            pending.extend(implied.iter().cloned());
        }
    }
    enabled
}

/// Report whether any target selector was given.
fn any_selector(options: &CargoOptions) -> bool {
    let flags = [
        options.lib,
        options.bins,
        options.tests,
        options.examples,
        options.benches,
    ];
    let name_lists = [
        &options.bin,
        &options.test,
        &options.example,
        &options.bench,
    ];

    flags.into_iter().any(|flag| flag) || name_lists.into_iter().any(|names| !names.is_empty())
}

/// Report whether `--lib` reaches the enabled targets.
fn library_selected(targets: &[&TargetFacts], options: &CargoOptions) -> bool {
    options.lib && targets.iter().any(|target| target.is_library())
}

/// Report whether a binary selector reaches the enabled targets.
fn binaries_selected(targets: &[&TargetFacts], options: &CargoOptions) -> bool {
    selected(targets, options.bins, TargetFacts::is_binary, &options.bin)
}

/// Report whether a test selector reaches the enabled targets.
fn tests_selected(targets: &[&TargetFacts], options: &CargoOptions) -> bool {
    selected(
        targets,
        options.tests,
        |target| target.has_test_harness,
        &options.test,
    )
}

/// Report whether an example selector reaches the enabled targets.
fn examples_selected(targets: &[&TargetFacts], options: &CargoOptions) -> bool {
    selected(
        targets,
        options.examples,
        TargetFacts::is_example,
        &options.example,
    )
}

/// Report whether a benchmark selector reaches the enabled targets.
///
/// `cargo metadata` does not report a target's `bench` flag, so eligibility
/// here follows the target kinds: `--benches` builds every benchmark target,
/// and Cargo's default `bench = true` on library and binary targets means those
/// count too. A manifest that sets `bench = false` on its library is therefore
/// over-listed; the run itself still follows Cargo.
fn benches_selected(targets: &[&TargetFacts], options: &CargoOptions) -> bool {
    selected(
        targets,
        options.benches,
        |target| target.is_bench() || target.is_library() || target.is_binary(),
        &options.bench,
    )
}

/// Combine an aggregate selector with the named selectors of the same kind.
///
/// Cargo's `--bins`, `--tests`, `--examples` and `--benches` select every
/// matching target, and their named forms select one target by pattern; a
/// package is visited when either form reaches an enabled target.
fn selected(
    targets: &[&TargetFacts],
    aggregate: bool,
    matches_kind: fn(&TargetFacts) -> bool,
    patterns: &[String],
) -> bool {
    (aggregate && targets.iter().any(|target| matches_kind(target)))
        || patterns
            .iter()
            .any(|pattern| named_target(targets, matches_kind, pattern))
}

/// Report whether a named selector matches an enabled target of the given kind.
fn named_target(
    targets: &[&TargetFacts],
    is_kind: fn(&TargetFacts) -> bool,
    pattern: &str,
) -> bool {
    targets
        .iter()
        .any(|target| is_kind(target) && glob::matches_name(pattern, &target.name))
}

#[cfg(test)]
#[path = "targets_tests.rs"]
mod tests;
