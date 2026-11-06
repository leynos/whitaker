//! Check metadata selection and manifest classification without spawning tools.
//!
//! Command routing lives in the sibling module; the property tests repeat both
//! the argument shapes and the classification outcomes over generated inputs.

use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
};

use tempfile::tempdir;

use super::{ManifestMatch, classify_manifest, is_workspace_metadata_query};

/// Prepared workspace the classifier tests compare against.
///
/// The path is real and holds a `Cargo.toml`, because the classifier resolves
/// the manifest before comparing it. Synthetic roots would exercise the
/// "missing manifest" path instead of a match.
const PREPARED_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Root manifest of [`PREPARED_ROOT`].
const PREPARED_MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

/// Joined `--manifest-path` form naming [`PREPARED_MANIFEST`].
const PREPARED_JOINED: &str = concat!(
    "--manifest-path=",
    env!("CARGO_MANIFEST_DIR"),
    "/Cargo.toml"
);

/// Report every manifest as readable.
fn readable(_: &Path) -> bool { true }

/// Require workspace metadata requests to match the prepared root.
#[test]
fn uses_cached_metadata_only_for_workspace_queries() {
    let arguments = ["metadata", "--no-deps"].map(OsString::from);

    let is_workspace_query = is_workspace_metadata_query(
        &arguments,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("workspace paths should resolve");
    let is_fixture_query = is_workspace_metadata_query(
        &arguments,
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples"),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("workspace path should resolve");

    assert!(is_workspace_query);
    assert!(!is_fixture_query);
}

/// Recognize the metadata query behind a rustup toolchain selector.
#[test]
fn uses_cached_metadata_behind_a_toolchain_selector() {
    let arguments = ["+nightly", "metadata", "--no-deps"].map(OsString::from);

    let is_workspace_query = is_workspace_metadata_query(
        &arguments,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("workspace paths should resolve");

    assert!(is_workspace_query);
}

/// Refuse the cache when the query asks for the dependency graph.
#[test]
fn refuses_cached_metadata_for_a_full_query() {
    let arguments = ["metadata", "--format-version", "1"].map(OsString::from);

    let is_workspace_query = is_workspace_metadata_query(
        &arguments,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("workspace paths should resolve");

    assert!(
        !is_workspace_query,
        "the prepared cache holds --no-deps output and must not answer a full query"
    );
}

/// Refuse the cache when a repeated selector names another workspace.
#[test]
fn refuses_cached_metadata_for_a_repeated_foreign_manifest() {
    let arguments = [
        "metadata",
        "--no-deps",
        "--manifest-path",
        "Cargo.toml",
        "--manifest-path",
        "/other/Cargo.toml",
    ]
    .map(OsString::from);

    let is_workspace_query = is_workspace_metadata_query(
        &arguments,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("workspace paths should resolve");

    assert!(!is_workspace_query);
}

/// Refuse the cache when a selector has no value.
#[test]
fn refuses_cached_metadata_for_a_dangling_manifest_flag() {
    let arguments = ["metadata", "--no-deps", "--manifest-path"].map(OsString::from);

    let is_workspace_query = is_workspace_metadata_query(
        &arguments,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("workspace paths should resolve");

    assert!(!is_workspace_query);
}

/// Refuse the cache when a selector names a manifest that is not there.
///
/// The prepared workspace's own manifest is readable, so only the selector is
/// in question. A lexical comparison would call the missing path a match and
/// answer from metadata describing a file that is gone; the classifier must
/// leave it to the real Cargo.
#[test]
fn refuses_cached_metadata_when_a_selector_names_a_missing_manifest() {
    let workspace = tempdir().expect("a temporary workspace should be created");
    fs::write(workspace.path().join("Cargo.toml"), b"[workspace]\n")
        .expect("a root manifest should be written");
    let arguments = [
        "metadata",
        "--no-deps",
        "--manifest-path",
        "missing/Cargo.toml",
    ]
    .map(OsString::from);

    let is_workspace_query =
        is_workspace_metadata_query(&arguments, workspace.path(), workspace.path())
            .expect("an existing workspace directory should resolve");

    assert!(!is_workspace_query);
}

/// Refuse the cache when the prepared workspace has no root manifest.
#[test]
fn refuses_cached_metadata_when_the_prepared_workspace_has_no_root_manifest() {
    let workspace = tempdir().expect("a temporary workspace should be created");
    let arguments = ["metadata", "--no-deps", "--manifest-path", "Cargo.toml"].map(OsString::from);

    let is_workspace_query =
        is_workspace_metadata_query(&arguments, workspace.path(), workspace.path())
            .expect("an existing workspace directory should resolve");

    assert!(!is_workspace_query);
}

/// Refuse the cache when a selector-free query has no root manifest.
///
/// A request without `--manifest-path` has no selector to classify, and
/// `all()` over an empty list is vacuously true. The workspace's own manifest
/// is the only thing left to check, so its absence must keep the query away
/// from the cache.
#[test]
fn refuses_cached_metadata_when_a_selector_free_query_has_no_root_manifest() {
    let workspace = tempdir().expect("a temporary workspace should be created");
    let arguments = ["metadata", "--no-deps"].map(OsString::from);

    let is_workspace_query =
        is_workspace_metadata_query(&arguments, workspace.path(), workspace.path())
            .expect("an existing workspace directory should resolve");

    assert!(!is_workspace_query);
}

/// Classify a readable root manifest as prepared.
#[test]
fn classifies_a_readable_root_manifest_as_prepared() {
    let matched = classify_manifest(
        OsStr::new(PREPARED_MANIFEST),
        Path::new(PREPARED_ROOT),
        readable,
    )
    .expect("the prepared root manifest should resolve");

    assert!(matches!(matched, ManifestMatch::Prepared), "{matched:?}");
}

/// Classify a sibling manifest that is not the prepared root as foreign.
#[test]
fn classifies_a_sibling_manifest_as_foreign() {
    let manifest = Path::new(PREPARED_ROOT).join("src/example.rs");

    let matched = classify_manifest(manifest.as_os_str(), Path::new(PREPARED_ROOT), readable)
        .expect("the sibling manifest should resolve");

    assert!(matches!(matched, ManifestMatch::Foreign), "{matched:?}");
}

/// Report a missing manifest as an I/O error, not as a plain non-match.
///
/// The distinction is what lets a caller tell "this names another workspace"
/// from "this path is broken"; both still route to the real Cargo.
#[test]
fn classifies_a_missing_manifest_as_unresolvable() {
    let workspace = tempdir().expect("a temporary workspace should be created");

    let error = classify_manifest(OsStr::new("missing/Cargo.toml"), workspace.path(), readable)
        .expect_err("a missing manifest should not resolve");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{error}");
}

/// Report a readable-on-paper manifest that cannot be opened as an I/O error.
///
/// Readability is injected so this branch is covered on every runner, including
/// a privileged one that ignores permission bits.
#[test]
fn classifies_an_unreadable_manifest_as_unresolvable() {
    let error = classify_manifest(
        OsStr::new(PREPARED_MANIFEST),
        Path::new(PREPARED_ROOT),
        |_| false,
    )
    .expect_err("an unreadable manifest should not be prepared");

    assert_eq!(
        error.kind(),
        std::io::ErrorKind::PermissionDenied,
        "{error}"
    );
}

#[path = "cargo_test_driver_routing_tests.rs"]
mod routing;

#[path = "cargo_test_driver_proptests.rs"]
mod proptests;
// weave: run 'weave explain vendor/dylint_testing/src/cargo_test_driver_tests.rs' for per-hunk
// detail, 'weave check' to verify your resolution
