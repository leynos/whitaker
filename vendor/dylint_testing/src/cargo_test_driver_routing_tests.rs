//! Check how the driver builds the child command it routes to.
//!
//! These tests inspect the `Command` the driver spawns rather than running it,
//! so they hold on every platform and need no toolchain on `PATH`.

use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
};

use rstest::rstest;
use tempfile::tempdir;

use super::{PREPARED_JOINED, PREPARED_MANIFEST, PREPARED_ROOT};
use crate::cargo_test_driver::{is_workspace_metadata_query, routed_command};

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

/// Keep the driver's own variables out of the routed Nextest child.
///
/// The assertion is made on the command the driver would spawn, because the
/// repository forbids mutating the process environment in tests. Inspecting
/// the explicit removal list is also stronger than observing an absence: it
/// separates "removed here" from "never set by the caller".
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
