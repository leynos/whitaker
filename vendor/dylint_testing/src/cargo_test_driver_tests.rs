//! Check metadata selection and command routing without spawning tools.
//!
//! The property test repeats the enumerated argument shapes over generated
//! arguments, so an unlisted ordering cannot slip through.

use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
};

use rstest::rstest;
use tempfile::tempdir;

use super::{is_workspace_metadata_query, manifest_names_prepared_root, routed_command};

/// Prepared workspace the routing tests compare against.
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

/// Keep the driver's own variables out of the routed Nextest child.
#[test]
fn removes_the_driver_variables_from_the_nextest_child() {
    let arguments = ["nextest", "run"].map(OsString::from);
    let command = routed_command(
        OsStr::new("cargo"),
        OsStr::new("cargo-nextest"),
        &arguments,
        None,
    );
    let removed: Vec<_> = command
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_owned())
        .collect();

    assert!(
        removed.contains(&OsString::from("CARGO_METADATA_CACHE")),
        "the child must not inherit the metadata cache: {removed:?}"
    );
    assert!(
        removed.contains(&OsString::from("CARGO_METADATA_ROOT")),
        "the child must not inherit the metadata root: {removed:?}"
    );
}

/// Preserve Cargo flags when routing its Nextest subcommand.
#[test]
fn routes_nextest_without_adding_frozen_mode() {
    let arguments = ["nextest", "run", "--locked", "--workspace"].map(OsString::from);
    let command = routed_command(
        OsStr::new("cargo"),
        OsStr::new("cargo-nextest"),
        &arguments,
        None,
    );
    let actual: Vec<_> = command.get_args().collect();

    assert_eq!(command.get_program(), OsStr::new("cargo-nextest"));
    assert_eq!(actual, ["nextest", "run", "--locked", "--workspace"]);
}

/// Route `nextest run` arguments against the prepared workspace root.
fn routed_arguments(arguments: &[&str]) -> Vec<String> {
    let arguments = arguments.iter().map(OsString::from).collect::<Vec<_>>();
    routed_command(
        OsStr::new("cargo"),
        OsStr::new("cargo-nextest"),
        &arguments,
        Some(Path::new(PREPARED_ROOT)),
    )
    .get_args()
    .map(|argument| argument.to_string_lossy().into_owned())
    .collect()
}

/// Drop only the manifest selectors that name the prepared workspace.
#[rstest]
#[case::separate_form(
    &[
        "nextest",
        "run",
        "--manifest-path",
        PREPARED_MANIFEST,
        "--target-dir",
        "/workspace/target/llvm-cov-target",
        "--cargo-metadata",
        "/workspace/target/metadata.json",
    ][..],
    &[
        "nextest",
        "run",
        "--target-dir",
        "/workspace/target/llvm-cov-target",
        "--cargo-metadata",
        "/workspace/target/metadata.json",
    ][..],
)]
#[case::joined_form(
    &["nextest", "run", PREPARED_JOINED][..],
    &["nextest", "run"][..],
)]
#[case::other_workspace(
    &["nextest", "run", "--manifest-path", "/other/Cargo.toml"][..],
    &["nextest", "run", "--manifest-path", "/other/Cargo.toml"][..],
)]
#[case::repeated_prepared(
    &[
        "nextest",
        "run",
        "--manifest-path",
        PREPARED_MANIFEST,
        PREPARED_JOINED,
    ][..],
    &["nextest", "run"][..],
)]
#[case::repeated_other_workspace(
    &[
        "nextest",
        "run",
        "--manifest-path",
        PREPARED_MANIFEST,
        "--manifest-path",
        "/other/Cargo.toml",
    ][..],
    &["nextest", "run", "--manifest-path", "/other/Cargo.toml"][..],
)]
#[case::dangling_flag(
    &["nextest", "run", "--manifest-path"][..],
    &["nextest", "run", "--manifest-path"][..],
)]
fn routes_nextest_around_the_prepared_manifest(
    #[case] arguments: &[&str],
    #[case] expected: &[&str],
) {
    assert_eq!(routed_arguments(arguments), expected);
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

/// Keep an unresolved manifest selector when routing Nextest.
///
/// The selector and the prepared metadata cannot travel together, but dropping
/// it would silently point Nextest at the prepared workspace when the caller
/// named a manifest that is not there.
#[test]
fn keeps_a_missing_manifest_when_routing_nextest() {
    let workspace = tempdir().expect("a temporary workspace should be created");
    let arguments = ["nextest", "run", "--manifest-path", "Cargo.toml"].map(OsString::from);

    let routed = routed_command(
        OsStr::new("cargo"),
        OsStr::new("cargo-nextest"),
        &arguments,
        Some(workspace.path()),
    );

    assert_eq!(
        routed.get_args().collect::<Vec<_>>(),
        ["nextest", "run", "--manifest-path", "Cargo.toml"],
        "an unresolved manifest must stay on the routed call"
    );
}

/// Treat an unreadable prepared manifest as foreign.
///
/// Readability is injected so this branch is covered on every runner, including
/// a privileged one that ignores permission bits. The production wiring is
/// checked separately by [`keeps_an_unreadable_manifest_when_routing_nextest`].
#[test]
fn refuses_an_unreadable_prepared_manifest() {
    let names_prepared = manifest_names_prepared_root(
        OsStr::new(PREPARED_MANIFEST),
        Path::new(PREPARED_ROOT),
        |_| false,
    );

    assert!(!names_prepared);
}

/// Keep an unreadable prepared manifest's selector when routing Nextest.
///
/// A privileged runner ignores the mode bits, so the test skips itself when the
/// manifest is still readable after the permission change rather than asserting
/// an outcome the environment cannot produce. Permissions are restored before
/// the temporary directory is removed.
#[cfg(unix)]
#[test]
fn keeps_an_unreadable_manifest_when_routing_nextest() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempdir().expect("a temporary workspace should be created");
    let manifest = workspace.path().join("Cargo.toml");
    fs::write(&manifest, b"[workspace]\n").expect("a root manifest should be written");
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o000))
        .expect("the manifest permissions should be changed");
    let is_enforced = fs::File::open(&manifest).is_err();
    let arguments = ["nextest", "run", "--manifest-path", "Cargo.toml"].map(OsString::from);
    // Route while the manifest is still unreadable, then restore the mode.
    let routed = is_enforced.then(|| {
        routed_command(
            OsStr::new("cargo"),
            OsStr::new("cargo-nextest"),
            &arguments,
            Some(workspace.path()),
        )
    });
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o644))
        .expect("the manifest permissions should be restored");

    let Some(routed) = routed else {
        return;
    };
    assert_eq!(
        routed.get_args().collect::<Vec<_>>(),
        ["nextest", "run", "--manifest-path", "Cargo.toml"],
        "an unreadable manifest must stay on the routed call"
    );
}

/// Pass ordinary Cargo calls through without forcing a lockfile policy.
#[test]
fn preserves_offline_trybuild_arguments() {
    let arguments = ["build", "--offline"].map(OsString::from);
    let command = routed_command(
        OsStr::new("cargo"),
        OsStr::new("cargo-nextest"),
        &arguments,
        None,
    );
    let actual: Vec<_> = command.get_args().collect();

    assert_eq!(command.get_program(), OsStr::new("cargo"));
    assert_eq!(actual, ["build", "--offline"]);
}

/// Recognize and route the prepared manifest through a `..` spelling.
#[test]
fn recognizes_a_manifest_spelled_through_a_parent_segment() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = workspace.join("src").join("..").join("Cargo.toml");
    let manifest = manifest
        .to_str()
        .expect("the manifest path should be UTF-8");
    let metadata = ["metadata", "--no-deps", "--manifest-path", manifest].map(OsString::from);
    let nextest = ["nextest", "run", "--manifest-path", manifest].map(OsString::from);

    let is_workspace_query = is_workspace_metadata_query(&metadata, workspace, workspace)
        .expect("workspace paths should resolve");
    let routed = routed_command(
        OsStr::new("cargo"),
        OsStr::new("cargo-nextest"),
        &nextest,
        Some(workspace),
    );

    assert!(
        is_workspace_query,
        "a non-canonical manifest must still match the prepared workspace"
    );
    assert_eq!(
        routed.get_args().collect::<Vec<_>>(),
        ["nextest", "run"],
        "a non-canonical prepared manifest must be dropped"
    );
}

#[path = "cargo_test_driver_proptests.rs"]
mod proptests;
// weave: run 'weave explain vendor/dylint_testing/src/cargo_test_driver_tests.rs' for per-hunk detail, 'weave check' to verify your resolution
