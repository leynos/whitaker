//! Selection rules over hand-built workspace facts.

use googletest::prelude::*;
use rstest::rstest;

use super::{SelectionError, checking_line, selected_packages};
use crate::{
    cli::check::CargoOptions,
    ports::{PackageFacts, PackageSpec, TargetFacts, WorkspaceFacts},
};

/// Build one library package with a manifest under `root`.
fn package(name: &str, version: &str, root: &str) -> PackageFacts {
    PackageFacts {
        name: name.to_owned(),
        version: version.to_owned(),
        manifest_path: format!("{root}/{name}/Cargo.toml").into(),
        targets: vec![TargetFacts {
            name: name.to_owned(),
            kinds: vec!["lib".to_owned()],
            required_features: Vec::new(),
            has_test_harness: true,
        }],
        features: std::collections::BTreeMap::default(),
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
#[case::unmatched_exclude_is_ignored(
    CargoOptions {
        workspace: true,
        excludes: vec!["absent".to_owned()],
        ..CargoOptions::default()
    },
    vec!["rootpkg", "member_a", "member_b"]
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
#[case::package_partial_version(
    CargoOptions {
        packages: vec!["member_a@0.1".to_owned()],
        ..CargoOptions::default()
    },
    vec!["member_a"]
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

/// Refuse a selector that names nothing, and refuse an orphaned exclusion.
#[rstest]
#[case::package(
    CargoOptions { packages: vec!["absent".to_owned()], ..CargoOptions::default() },
    SelectionError::UnknownPackage { spec: PackageSpec::from("absent") }
)]
#[case::unresolved_exclude(
    CargoOptions {
        workspace: true,
        excludes: vec!["path+file:///work%zz/member_a#member_a".to_owned()],
        ..CargoOptions::default()
    },
    SelectionError::UnresolvedSelector {
        spec: PackageSpec::from("path+file:///work%zz/member_a#member_a")
    }
)]
#[case::unknown_package_in_workspace_mode(
    CargoOptions {
        workspace: true,
        packages: vec!["absent".to_owned()],
        ..CargoOptions::default()
    },
    SelectionError::UnknownPackage { spec: PackageSpec::from("absent") }
)]
#[case::exclude_without_workspace(
    CargoOptions {
        excludes: vec!["member_a".to_owned()],
        ..CargoOptions::default()
    },
    SelectionError::ExcludeWithoutWorkspace
)]
fn invalid_selectors_are_reported(#[case] options: CargoOptions, #[case] expected: SelectionError) {
    let error =
        selected_packages(&facts(), &options).expect_err("an invalid selection must be reported");

    assert_eq!(error, expected, "the error must name the selector");
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
