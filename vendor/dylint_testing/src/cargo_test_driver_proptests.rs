//! Check argument routing and cache selection over generated inputs.
//!
//! The enumerated tests pin down each flag form; these properties repeat the
//! same invariants over arbitrary orderings of that grammar.

use std::{
    ffi::{OsStr, OsString},
    fs,
    path::Path,
};

use proptest::prelude::*;

use super::{
    super::without_prepared_manifest,
    PREPARED_JOINED,
    PREPARED_MANIFEST,
    PREPARED_ROOT,
    is_workspace_metadata_query,
};

/// Report whether a manifest value names the prepared workspace manifest.
///
/// Cargo resolves a relative `--manifest-path` against the workspace, so
/// `Cargo.toml` names the same file as `<workspace>/Cargo.toml`. Both sides
/// are resolved here, independently of the router's classifier, so a match
/// means the same file rather than the same spelling.
fn names_prepared_manifest(value: &OsStr, workspace: &Path) -> bool {
    let manifest = Path::new(value);
    let manifest = if manifest.is_absolute() {
        manifest.to_path_buf()
    } else {
        workspace.join(manifest)
    };

    match (fs::canonicalize(manifest), fs::canonicalize(workspace)) {
        (Ok(manifest), Ok(workspace)) => manifest == workspace.join("Cargo.toml"),
        _ => false,
    }
}

/// Build the arguments routing must forward for the prepared workspace.
///
/// This specification is written against the prepared path rather than the
/// router's helpers, so the property test can disagree with the implementation.
fn expected_forwarded(arguments: &[OsString], workspace: &Path) -> Vec<OsString> {
    let mut expected = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument == OsStr::new("--manifest-path") {
            match arguments.get(index + 1) {
                Some(value) if names_prepared_manifest(value, workspace) => index += 2,
                Some(value) => {
                    expected.push(argument.clone());
                    expected.push(value.clone());
                    index += 2;
                }
                None => {
                    expected.push(argument.clone());
                    index += 1;
                }
            }
            continue;
        }
        if argument
            .to_string_lossy()
            .strip_prefix("--manifest-path=")
            .is_some_and(|manifest| names_prepared_manifest(OsStr::new(manifest), workspace))
        {
            index += 1;
            continue;
        }
        expected.push(argument.clone());
        index += 1;
    }

    expected
}

/// Report whether any selector still names the prepared workspace.
///
/// A token following a separate flag is that flag's value, so it is never
/// itself a selector, even when it is spelled like one.
fn contains_prepared_selector(arguments: &[OsString], workspace: &Path) -> bool {
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument == OsStr::new("--manifest-path") {
            index += 2;
            continue;
        }
        if argument
            .to_string_lossy()
            .strip_prefix("--manifest-path=")
            .is_some_and(|manifest| names_prepared_manifest(OsStr::new(manifest), workspace))
        {
            return true;
        }
        index += 1;
    }

    false
}

/// Produce one argument from the flag grammar the router must respect.
fn arbitrary_argument() -> impl Strategy<Value = OsString> {
    prop_oneof![
        Just(OsString::from("--manifest-path")),
        Just(OsString::from(PREPARED_JOINED)),
        Just(OsString::from(PREPARED_MANIFEST)),
        Just(OsString::from("--manifest-path=/other/Cargo.toml")),
        Just(OsString::from("/other/Cargo.toml")),
        Just(OsString::from("Cargo.toml")),
        "[a-z]{0,8}".prop_map(OsString::from),
    ]
}

proptest! {
    /// Routing removes exactly the prepared selectors, in every ordering.
    #[test]
    fn routing_removes_only_the_prepared_manifest(
        arguments in proptest::collection::vec(arbitrary_argument(), 0..12),
    ) {
        let workspace = Path::new(PREPARED_ROOT);

        let forwarded = without_prepared_manifest(&arguments, Some(workspace));

        let expected = expected_forwarded(&arguments, workspace);
        prop_assert_eq!(&forwarded, &expected);
        prop_assert!(forwarded.len() <= arguments.len());
        prop_assert!(!contains_prepared_selector(&forwarded, workspace));
        prop_assert_eq!(
            without_prepared_manifest(&forwarded, Some(workspace)),
            forwarded,
            "a second pass must not remove anything"
        );
    }

    /// Without a prepared workspace every argument passes through unchanged.
    #[test]
    fn routing_without_a_workspace_preserves_every_argument(
        arguments in proptest::collection::vec(arbitrary_argument(), 0..12),
    ) {
        prop_assert_eq!(without_prepared_manifest(&arguments, None), arguments);
    }
}

/// Produce one argument from the metadata-query grammar.
///
/// The tokens cover the subcommand, the `--no-deps` marker, both selector
/// forms, a foreign and a missing manifest, and a rustup toolchain selector.
fn arbitrary_metadata_argument() -> impl Strategy<Value = OsString> {
    prop_oneof![
        Just(OsString::from("metadata")),
        Just(OsString::from("nextest")),
        Just(OsString::from("build")),
        Just(OsString::from("--no-deps")),
        Just(OsString::from("--format-version")),
        Just(OsString::from("1")),
        Just(OsString::from("--manifest-path")),
        Just(OsString::from(PREPARED_MANIFEST)),
        Just(OsString::from(PREPARED_JOINED)),
        Just(OsString::from("--manifest-path=/other/Cargo.toml")),
        Just(OsString::from("/other/Cargo.toml")),
        Just(OsString::from("Cargo.toml")),
        Just(OsString::from("missing/Cargo.toml")),
        Just(OsString::from("+nightly")),
        "[a-z]{0,8}".prop_map(OsString::from),
    ]
}

/// Report whether every selector after the subcommand names the prepared root.
///
/// This is the contract the cache depends on, written against the flag grammar
/// rather than the driver's parser, so the property can disagree with the
/// implementation. A dangling flag names nothing and is therefore foreign.
fn every_selector_names_prepared(
    arguments: &[OsString],
    command_index: usize,
    workspace: &Path,
) -> bool {
    let mut index = command_index + 1;
    while let Some(argument) = arguments.get(index) {
        if argument == OsStr::new("--manifest-path") {
            let Some(value) = arguments.get(index + 1) else {
                return false;
            };
            if !names_prepared_manifest(value, workspace) {
                return false;
            }
            index += 2;
            continue;
        }
        if let Some(value) = argument.to_string_lossy().strip_prefix("--manifest-path=")
            && !names_prepared_manifest(OsStr::new(value), workspace)
        {
            return false;
        }
        index += 1;
    }

    true
}

/// Decide cache eligibility independently of the driver's classifier.
fn expected_cache_hit(arguments: &[OsString], workspace: &Path) -> bool {
    let command_index = usize::from(
        arguments
            .first()
            .is_some_and(|argument| argument.to_string_lossy().starts_with('+')),
    );
    let is_no_deps_metadata = arguments.get(command_index) == Some(&OsString::from("metadata"))
        && arguments
            .iter()
            .any(|argument| argument == OsStr::new("--no-deps"));

    is_no_deps_metadata
        && workspace.join("Cargo.toml").is_file()
        && every_selector_names_prepared(arguments, command_index, workspace)
}

proptest! {
    /// Serve the prepared cache exactly when the oracle says the query matches.
    #[test]
    fn cache_selection_matches_the_selector_contract(
        arguments in proptest::collection::vec(arbitrary_metadata_argument(), 0..12),
    ) {
        let workspace = Path::new(PREPARED_ROOT);

        let served = is_workspace_metadata_query(&arguments, workspace, workspace)
            .expect("the prepared workspace should resolve");

        prop_assert_eq!(served, expected_cache_hit(&arguments, workspace));
    }
}
// weave: run 'weave explain vendor/dylint_testing/src/cargo_test_driver_proptests.rs' for per-hunk
// detail, 'weave check' to verify your resolution
