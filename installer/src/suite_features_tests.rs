//! Tests for choosing the experimental features a suite declares.

use camino::Utf8PathBuf;
use rstest::rstest;
use tempfile::TempDir;

use super::*;

/// Write `suite/Cargo.toml` with `manifest` under a fresh workspace root.
fn workspace_with_suite(manifest: &str) -> (TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().expect("temporary workspace");
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("UTF-8 path");
    std::fs::create_dir_all(root.join("suite")).expect("create suite directory");
    std::fs::write(root.join("suite/Cargo.toml"), manifest).expect("write suite manifest");
    (dir, root)
}

/// A suite from 0.3.0 onward is switched on by the single aggregate feature.
#[rstest]
fn a_current_suite_gets_only_the_aggregate_feature() {
    let (_dir, root) = workspace_with_suite(concat!(
        "[features]\n",
        "dylint-driver = []\n",
        "experimental-enable-all = [\"dylint-driver\"]\n",
        "experimental-macro-aware-cognitive-complexity = [\"dylint-driver\"]\n",
    ));
    assert_eq!(
        experimental_features(&root).expect("manifest should parse"),
        "experimental-enable-all"
    );
}

/// A pinned older suite is switched on by the per-lint features it declares.
#[rstest]
fn a_pinned_suite_without_the_aggregate_gets_its_declared_per_lint_features() {
    let (_dir, root) = workspace_with_suite(concat!(
        "[features]\n",
        "dylint-driver = []\n",
        "experimental-rstest-helper-should-be-fixture = [\"dylint-driver\"]\n",
    ));
    assert_eq!(
        experimental_features(&root).expect("manifest should parse"),
        "experimental-rstest-helper-should-be-fixture"
    );
}

/// A suite that declares no experimental feature gets none, rather than a
/// feature Cargo would reject.
#[rstest]
fn a_suite_without_experimental_features_gets_none() {
    let (_dir, root) = workspace_with_suite("[features]\ndylint-driver = []\n");
    assert_eq!(
        experimental_features(&root).expect("manifest should parse"),
        ""
    );
}

/// An unreadable or malformed manifest is an error, not "no features".
#[rstest]
fn a_malformed_manifest_is_an_error() {
    let (_dir, root) = workspace_with_suite("[features\n");
    let error = experimental_features(&root).expect_err("malformed manifest must fail");
    assert!(
        matches!(error, InstallerError::InvalidCargoToml { .. }),
        "got {error:?}"
    );
}

/// A missing manifest surfaces the I/O error.
#[rstest]
fn a_missing_manifest_is_an_error() {
    let dir = tempfile::tempdir().expect("temporary workspace");
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("UTF-8 path");
    let error = experimental_features(&root).expect_err("missing manifest must fail");
    assert!(matches!(error, InstallerError::Io(_)), "got {error:?}");
}
