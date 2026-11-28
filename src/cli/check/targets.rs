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

use super::{cargo::CargoOptions, glob};
use crate::ports::{PackageFacts, TargetFacts};

/// Report whether the target selectors leave work for `package`.
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
///     }],
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
    if package.targets.is_empty() || options.all_targets {
        return true;
    }
    if any_selector(options) {
        return selector_matches(package, options);
    }
    package.has_library() || package.has_binary()
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

/// Report whether any selector reaches the package's targets.
fn selector_matches(package: &PackageFacts, options: &CargoOptions) -> bool {
    library_selected(package, options)
        || binaries_selected(package, options)
        || tests_selected(package, options)
        || examples_selected(package, options)
        || benches_selected(package, options)
}

/// Report whether `--lib` reaches this package.
fn library_selected(package: &PackageFacts, options: &CargoOptions) -> bool {
    options.lib && package.has_library()
}

/// Report whether a binary selector reaches this package.
///
/// A named selector is a glob pattern in Cargo, so `--bin 'tool-*'` reaches a
/// binary called `tool-cli`.
fn binaries_selected(package: &PackageFacts, options: &CargoOptions) -> bool {
    (options.bins && package.has_binary())
        || options
            .bin
            .iter()
            .any(|pattern| named_target(package, TargetFacts::is_binary, pattern))
}

/// Report whether a test selector reaches this package.
///
/// `--tests` also builds the implicit unit-test target of a library or binary,
/// so a package with either target has work under it even without an
/// integration-test target of its own.
fn tests_selected(package: &PackageFacts, options: &CargoOptions) -> bool {
    (options.tests && (package.has_library() || package.has_binary() || package.has_test()))
        || options
            .test
            .iter()
            .any(|pattern| named_target(package, TargetFacts::is_test, pattern))
}

/// Report whether an example selector reaches this package.
fn examples_selected(package: &PackageFacts, options: &CargoOptions) -> bool {
    (options.examples && package.has_example())
        || options
            .example
            .iter()
            .any(|pattern| named_target(package, TargetFacts::is_example, pattern))
}

/// Report whether a benchmark selector reaches this package.
///
/// `--benches` also builds the implicit benchmark target of a library or
/// binary, which Cargo enables by default, so a package with either target has
/// work under it even without a `[[bench]]` target of its own.
fn benches_selected(package: &PackageFacts, options: &CargoOptions) -> bool {
    (options.benches && (package.has_bench() || package.has_library() || package.has_binary()))
        || options
            .bench
            .iter()
            .any(|pattern| named_target(package, TargetFacts::is_bench, pattern))
}

/// Report whether a named selector matches a target of the given kind.
fn named_target(package: &PackageFacts, is_kind: fn(&TargetFacts) -> bool, pattern: &str) -> bool {
    package
        .targets
        .iter()
        .any(|target| is_kind(target) && glob::matches_name(pattern, &target.name))
}

#[cfg(test)]
mod tests {
    //! Eligibility over packages with known targets.

    use rstest::rstest;

    use super::is_checked;
    use crate::{
        cli::check::CargoOptions,
        ports::{PackageFacts, TargetFacts},
    };

    /// Build a package with the named targets, each `(name, kind)`.
    fn package(targets: &[(&str, &str)]) -> PackageFacts {
        PackageFacts {
            name: "app".to_owned(),
            version: "0.1.0".to_owned(),
            manifest_path: "/work/app/Cargo.toml".into(),
            targets: targets
                .iter()
                .map(|(name, kind)| TargetFacts {
                    name: (*name).to_owned(),
                    kinds: vec![(*kind).to_owned()],
                })
                .collect(),
        }
    }

    /// A package with only a library.
    fn library() -> PackageFacts { package(&[("app", "lib")]) }

    /// A package with a library and a binary named `tool`.
    fn library_and_tool() -> PackageFacts { package(&[("app", "lib"), ("tool", "bin")]) }

    /// A package with a library, an example and a benchmark.
    fn with_example_and_bench() -> PackageFacts {
        package(&[("app", "lib"), ("demo", "example"), ("load", "bench")])
    }

    /// Report the eligibility of each package under each selector.
    #[rstest]
    #[case::default_library(library(), CargoOptions::default(), true)]
    #[case::default_library_and_bin(library_and_tool(), CargoOptions::default(), true)]
    #[case::all_targets(
        library(),
        CargoOptions { all_targets: true, ..CargoOptions::default() },
        true
    )]
    #[case::lib_matches(library(), CargoOptions { lib: true, ..CargoOptions::default() }, true)]
    #[case::bins_reject_library(
        library(),
        CargoOptions { bins: true, ..CargoOptions::default() },
        false
    )]
    #[case::bins_match_binary(
        library_and_tool(),
        CargoOptions { bins: true, ..CargoOptions::default() },
        true
    )]
    #[case::named_bin_matches(
        library_and_tool(),
        CargoOptions { bin: vec!["tool".to_owned()], ..CargoOptions::default() },
        true
    )]
    #[case::named_bin_rejects(
        library(),
        CargoOptions { bin: vec!["tool".to_owned()], ..CargoOptions::default() },
        false
    )]
    #[case::named_bin_glob_matches(
        library_and_tool(),
        CargoOptions { bin: vec!["to*".to_owned()], ..CargoOptions::default() },
        true
    )]
    #[case::named_bin_glob_rejects(
        library_and_tool(),
        CargoOptions { bin: vec!["other*".to_owned()], ..CargoOptions::default() },
        false
    )]
    #[case::tests_reach_implicit_unit_tests(
        library(),
        CargoOptions { tests: true, ..CargoOptions::default() },
        true
    )]
    #[case::named_test_rejects_library(
        library(),
        CargoOptions { test: vec!["smoke".to_owned()], ..CargoOptions::default() },
        false
    )]
    #[case::examples_need_an_example(
        library(),
        CargoOptions { examples: true, ..CargoOptions::default() },
        false
    )]
    #[case::examples_match(
        with_example_and_bench(),
        CargoOptions { examples: true, ..CargoOptions::default() },
        true
    )]
    #[case::named_example_matches(
        with_example_and_bench(),
        CargoOptions { example: vec!["demo".to_owned()], ..CargoOptions::default() },
        true
    )]
    #[case::benches_reach_implicit_targets(
        library(),
        CargoOptions { benches: true, ..CargoOptions::default() },
        true
    )]
    #[case::benches_match(
        with_example_and_bench(),
        CargoOptions { benches: true, ..CargoOptions::default() },
        true
    )]
    #[case::named_bench_matches(
        with_example_and_bench(),
        CargoOptions { bench: vec!["load".to_owned()], ..CargoOptions::default() },
        true
    )]
    fn target_selectors_decide_eligibility(
        #[case] package: PackageFacts,
        #[case] options: CargoOptions,
        #[case] expected: bool,
    ) {
        assert_eq!(
            is_checked(&package, &options),
            expected,
            "eligibility for {options:?}"
        );
    }

    /// Keep a package whose facts carry no target information.
    #[rstest]
    #[case::default(CargoOptions::default())]
    #[case::bins(CargoOptions { bins: true, ..CargoOptions::default() })]
    fn missing_target_facts_do_not_hide_coverage(#[case] options: CargoOptions) {
        let package = PackageFacts {
            name: "app".to_owned(),
            version: "0.1.0".to_owned(),
            manifest_path: "/work/app/Cargo.toml".into(),
            targets: Vec::new(),
        };

        assert!(
            is_checked(&package, &options),
            "unknown targets must not exclude a package, got {options:?}"
        );
    }
}
