//! Check argument routing over generated inputs.
//!
//! The enumerated tests pin down each flag form; these properties repeat the
//! same invariant over arbitrary orderings of that grammar.

use super::super::without_prepared_manifest;
use proptest::prelude::*;
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

/// Report whether a manifest value names the prepared workspace manifest.
///
/// Cargo resolves a relative `--manifest-path` against the workspace, so
/// `Cargo.toml` names the same file as `<workspace>/Cargo.toml`. The
/// comparison stays lexical: the generated workspaces do not exist, so no
/// filesystem resolution is available to fold a `..` segment.
fn names_prepared_manifest(value: &OsStr, workspace: &Path) -> bool {
    let manifest = Path::new(value);
    let manifest = if manifest.is_absolute() {
        manifest.to_path_buf()
    } else {
        workspace.join(manifest)
    };

    manifest == workspace.join("Cargo.toml")
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
        Just(OsString::from("--manifest-path=/workspace/Cargo.toml")),
        Just(OsString::from("/workspace/Cargo.toml")),
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
        let workspace = Path::new("/workspace");

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
