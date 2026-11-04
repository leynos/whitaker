//! Check metadata selection and command routing without spawning tools.
//!
//! The property test repeats the enumerated argument shapes over generated
//! arguments, so an unlisted ordering cannot slip through.

use super::{is_workspace_metadata_query, routed_command};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use rstest::rstest;

//! Check metadata selection and command routing without spawning tools.
//!
//! The property test repeats the enumerated argument shapes over generated
//! arguments, so an unlisted ordering cannot slip through.
};

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

/// Route `nextest run` arguments against the prepared `/workspace` root.
fn routed_arguments(arguments: &[&str]) -> Vec<String> {
    let arguments = arguments.iter().map(OsString::from).collect::<Vec<_>>();
    routed_command(
        OsStr::new("cargo"),
        OsStr::new("cargo-nextest"),
        &arguments,
        Some(Path::new("/workspace")),
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
        "/workspace/Cargo.toml",
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
    &["nextest", "run", "--manifest-path=/workspace/Cargo.toml"][..],
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
        "/workspace/Cargo.toml",
        "--manifest-path=/workspace/Cargo.toml",
    ][..],
    &["nextest", "run"][..],
)]
#[case::repeated_other_workspace(
    &[
        "nextest",
        "run",
        "--manifest-path",
        "/workspace/Cargo.toml",
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
