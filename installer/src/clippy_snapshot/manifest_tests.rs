//! Validates snapshot manifest parsing and its agreement with the repository.

use super::*;
use crate::dependency_binaries::find_dependency_binary;
use rstest::rstest;

const VALID: &str = concat!(
    "schema_version = 1\n",
    "upstream = \"https://github.com/rust-lang/rust-clippy\"\n",
    "branch = \"main\"\n",
    "commit = \"e4608a25c2e20116fbc1e97a51a73f094a1db7a1\"\n",
    "toolchain = \"nightly-2026-05-28\"\n",
    "selected_revision = \"9fca3bc9fc2bc83c60bde26d18ed68f11564b228\"\n",
    "dylint_version = \"6.3.0\"\n",
);

#[test]
fn embedded_manifest_parses() {
    let manifest = embedded_manifest().expect("the committed manifest must be valid");
    assert_eq!(
        manifest.upstream,
        "https://github.com/rust-lang/rust-clippy"
    );
}

/// The snapshot is selected for the repository's own nightly; a drifted pin
/// would make every driver build select a revision nobody verified.
#[test]
fn embedded_toolchain_matches_rust_toolchain_file() {
    let toolchain_file = include_str!("../../../rust-toolchain.toml");
    let manifest = embedded_manifest().expect("the committed manifest must be valid");
    let expected = format!("channel = \"{}\"", manifest.toolchain);
    assert!(
        toolchain_file.lines().any(|line| line.trim() == expected),
        "rust-toolchain.toml must pin {}",
        manifest.toolchain
    );
}

/// `selected_revision` records one Dylint release's selection algorithm, so
/// it must name the release the installer provisions.
#[test]
fn embedded_dylint_version_matches_dependency_manifest() {
    let manifest = embedded_manifest().expect("the committed manifest must be valid");
    let cargo_dylint = find_dependency_binary("cargo-dylint")
        .expect("dependency manifest must parse")
        .expect("cargo-dylint must be listed");
    assert_eq!(manifest.dylint_version, cargo_dylint.version());
}

#[test]
fn asset_names_are_keyed_by_commit() {
    let manifest = parse_manifest(VALID).expect("valid manifest");
    assert_eq!(
        manifest.bundle_asset_name(),
        "whitaker-clippy-snapshot-e4608a25c2e20116fbc1e97a51a73f094a1db7a1.bundle"
    );
    assert_eq!(
        manifest.sidecar_asset_name(),
        "whitaker-clippy-snapshot-e4608a25c2e20116fbc1e97a51a73f094a1db7a1.bundle.sha256"
    );
}

#[rstest]
#[case::future_schema("schema_version = 1", "schema_version = 2", "schema_version 2")]
#[case::legacy_branch("branch = \"main\"", "branch = \"master\"", "branch `master`")]
#[case::short_commit(
    "commit = \"e4608a25c2e20116fbc1e97a51a73f094a1db7a1\"",
    "commit = \"e4608a25\"",
    "commit:"
)]
#[case::uppercase_revision(
    "selected_revision = \"9fca3bc9fc2bc83c60bde26d18ed68f11564b228\"",
    "selected_revision = \"9FCA3BC9FC2BC83C60BDE26D18ED68F11564B228\"",
    "selected_revision:"
)]
#[case::undated_toolchain(
    "toolchain = \"nightly-2026-05-28\"",
    "toolchain = \"nightly\"",
    "dated nightly"
)]
#[case::unknown_field(
    "dylint_version = \"6.3.0\"",
    "dylint_version = \"6.3.0\"\nrev = \"x\"",
    "unknown field"
)]
fn rejects_invalid_manifests(#[case] from: &str, #[case] to: &str, #[case] expected: &str) {
    let contents = VALID.replace(from, to);
    let error = parse_manifest(&contents).expect_err("manifest should be rejected");
    assert!(
        error.to_string().contains(expected),
        "`{error}` should mention `{expected}`"
    );
}
