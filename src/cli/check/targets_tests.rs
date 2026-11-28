//! Eligibility tests for the coverage listing.

use std::collections::BTreeMap;

use rstest::rstest;

use super::is_checked;
use crate::{
    cli::check::CargoOptions,
    ports::{PackageFacts, TargetFacts},
};

/// Build a package with the named targets, each `(name, kind)`.
///
/// Every target carries the default harness flags and no required
/// features; the eligibility cases that need otherwise build their own.
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
                required_features: Vec::new(),
                has_test_harness: true,
            })
            .collect(),
        features: BTreeMap::new(),
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
#[case::tests_follow_the_harness_flag(
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
#[case::benches_follow_the_harness_flag(
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

/// A target can turn its test harness off.
#[test]
fn a_disabled_test_harness_does_not_make_a_package_eligible() {
    let mut package = library();
    for target in &mut package.targets {
        target.has_test_harness = false;
    }
    let options = CargoOptions {
        tests: true,
        ..CargoOptions::default()
    };

    assert!(
        !is_checked(&package, &options),
        "a package with no remaining test harness has no work"
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
        features: BTreeMap::new(),
    };

    assert!(
        is_checked(&package, &options),
        "unknown targets must not exclude a package, got {options:?}"
    );
}

/// Required features gate a target exactly as they gate Cargo.
#[rstest]
#[case::disabled(BTreeMap::new(), CargoOptions::default(), false)]
#[case::named(
    BTreeMap::from([("cli".to_owned(), Vec::new())]),
    CargoOptions { features: vec!["cli".to_owned()], ..CargoOptions::default() },
    true
)]
#[case::all_features(
    BTreeMap::from([("cli".to_owned(), Vec::new())]),
    CargoOptions { all_features: true, ..CargoOptions::default() },
    true
)]
#[case::default_feature_satisfies(
    BTreeMap::from([
        ("default".to_owned(), vec!["cli".to_owned()]),
        ("cli".to_owned(), Vec::new()),
    ]),
    CargoOptions::default(),
    true
)]
#[case::default_feature_disabled(
    BTreeMap::from([
        ("default".to_owned(), vec!["cli".to_owned()]),
        ("cli".to_owned(), Vec::new()),
    ]),
    CargoOptions { no_default_features: true, ..CargoOptions::default() },
    false
)]
fn required_features_gate_a_target(
    #[case] features: BTreeMap<String, Vec<String>>,
    #[case] options: CargoOptions,
    #[case] expected: bool,
) {
    let mut package = package(&[("tool", "bin")]);
    package.features = features;
    if let Some(target) = package.targets.first_mut() {
        target.required_features = vec!["cli".to_owned()];
    }

    assert_eq!(
        is_checked(&package, &options),
        expected,
        "a required feature must gate the target: {options:?}"
    );
}
