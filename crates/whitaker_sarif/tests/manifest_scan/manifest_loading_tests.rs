//! Behavioural tests for loading a located manifest, and for discovery's
//! propagation of a load failure.
//!
//! [`load_manifest`] is the single route by which discovery turns bytes into a
//! table, and both the root and a member manifest travel it: `declared_path`
//! and `workspace_dependencies` load the root, and `assert_no_edge` loads
//! whichever member it is given. Until these cases existed, an unreadable or
//! unparsable manifest panicked behind a `Result` signature — a query that
//! promised to report a failure aborted its caller instead.
//!
//! **Two separate claims are pinned here, and they fail differently.** The
//! first four cases show that the loader *reports* a bad manifest. The last
//! three show that the two discovery queries *forward* that report. A
//! `declared_path_in` that swallowed the error and answered `Ok(None)` would
//! send every lookup down the directory convention, silently discarding the
//! authoritative source, while the loader cases stayed green.
//!
//! The functions under test take the workspace root as an argument for exactly
//! this reason: the real root cannot be made unreadable or invalid, so a test
//! that wants to watch the failure travel must supply a root it controls. Both
//! queries then run the same `?` on the same call, which is what makes the
//! propagation cases a check on the propagation rather than on the loader a
//! second time.
//!
//! Fixture writes go through the shared [`Scratch`] handle; see its module
//! documentation for why staging cannot happen under `CARGO_MANIFEST_DIR`.

use super::{
    DiscoveryError,
    declared_path_in,
    load_manifest,
    scratch::{Scratch, scratch},
    workspace_dependencies_in,
};

/// Joins `name` onto the scratch root.
///
/// A thin local alias for [`Scratch::path`], kept because every case in this
/// module reads `at(&scratch, "…")` and inlining the method there would bury
/// the fixture path in punctuation.
fn at(scratch: &Scratch, name: &str) -> camino::Utf8PathBuf { scratch.path(name) }

// -- A located manifest that cannot be loaded is an error, not a panic ------

#[test]
fn an_invalid_manifest_is_reported_with_its_path() {
    // Produced, not modelled: the file really is not TOML.
    let scratch = scratch();
    scratch
        .root
        .write("Cargo.toml", "[dependencies\nnot toml")
        .expect("the fixture manifest should be writable");

    let manifest = at(&scratch, "Cargo.toml");
    let error = load_manifest(&manifest).expect_err("invalid TOML must not load");

    assert!(
        matches!(
            error,
            DiscoveryError::ManifestInvalid { ref path, .. }
                if path.as_path() == manifest.as_path()
        ),
        "invalid TOML must be reported as such and name the file, got {error}"
    );
}

#[test]
fn a_manifest_in_place_of_a_directory_entry_is_a_read_error() {
    // A located "manifest" that is really a directory. Reading it refuses the
    // call, and the refusal must arrive as an error rather than a panic.
    let scratch = scratch();
    scratch
        .root
        .create_dir("Cargo.toml")
        .expect("the fixture directory should be creatable");

    let manifest = at(&scratch, "Cargo.toml");
    let error = load_manifest(&manifest).expect_err("a directory is not a loadable manifest");

    assert!(
        matches!(error, DiscoveryError::ManifestUnreadable { .. }),
        "an unreadable manifest must be reported as such, got {error}"
    );
}

#[test]
fn a_missing_manifest_is_a_read_error() {
    // `probe` would have called this absent before loading; here the load is
    // asked directly, and a missing file must still be an error rather than a
    // panic or an empty table.
    let scratch = scratch();
    let manifest = at(&scratch, "Cargo.toml");

    let error = load_manifest(&manifest).expect_err("a missing manifest cannot load");

    assert!(
        matches!(error, DiscoveryError::ManifestUnreadable { .. }),
        "a missing manifest must be reported as unreadable, got {error}"
    );
}

#[test]
fn a_loadable_manifest_becomes_a_table() {
    // The positive case: without it the three rejections above are satisfied by
    // a loader that fails on every input.
    let scratch = scratch();
    scratch
        .root
        .write("Cargo.toml", "[dependencies]\nserde = \"1\"\n")
        .expect("the fixture manifest should be writable");

    let document = load_manifest(&at(&scratch, "Cargo.toml")).expect("a valid manifest must load");

    assert!(
        document.get("dependencies").is_some(),
        "the loaded table must carry the dependency table it was given"
    );
}

// -- Discovery propagates a root failure, rather than only the loader ------

#[test]
fn discovery_propagates_an_invalid_root_manifest() {
    let scratch = scratch();
    scratch
        .root
        .write("Cargo.toml", "[workspace.dependencies\nnot toml")
        .expect("the fixture root manifest should be writable");

    let root = at(&scratch, ".");
    let declared = declared_path_in("whitaker-common", &root)
        .expect_err("an invalid root must not be read as 'the root declares nothing'");

    assert!(
        matches!(declared, DiscoveryError::ManifestInvalid { .. }),
        "declared_path must propagate the root's parse failure, got {declared}"
    );

    let workspace = workspace_dependencies_in(&root)
        .expect_err("an invalid root must not be read as 'the root declares nothing'");

    assert!(
        matches!(workspace, DiscoveryError::ManifestInvalid { .. }),
        "workspace_dependencies must propagate the root's parse failure, got {workspace}"
    );
}

#[test]
fn discovery_propagates_an_unreadable_root_manifest() {
    // A directory where the root manifest should be. Both queries must report
    // it rather than falling back to the convention with `Ok(None)`.
    let scratch = scratch();
    scratch
        .root
        .create_dir("Cargo.toml")
        .expect("the fixture directory should be creatable");

    let root = at(&scratch, ".");
    let declared = declared_path_in("whitaker-common", &root)
        .expect_err("an unreadable root must not be read as absence");

    assert!(
        matches!(declared, DiscoveryError::ManifestUnreadable { .. }),
        "declared_path must propagate the root's read failure, got {declared}"
    );

    let workspace = workspace_dependencies_in(&root)
        .expect_err("an unreadable root must not be read as absence");

    assert!(
        matches!(workspace, DiscoveryError::ManifestUnreadable { .. }),
        "workspace_dependencies must propagate the root's read failure, got {workspace}"
    );
}

#[test]
fn discovery_reads_a_valid_root_manifest() {
    // The positive case for both queries: a root that loads must yield the
    // declaration it contains, so the two propagation cases above cannot be
    // satisfied by functions that fail on every input.
    let scratch = scratch();
    scratch
        .root
        .write(
            "Cargo.toml",
            concat!(
                "[workspace.dependencies]\n",
                "whitaker-common = { path = \"common\" }\n",
            ),
        )
        .expect("the fixture root manifest should be writable");

    let root = at(&scratch, ".");
    let declared = declared_path_in("whitaker-common", &root)
        .expect("a valid root is readable")
        .expect("the root declares a path for the crate");

    assert_eq!(
        declared,
        at(&scratch, "common").join("Cargo.toml"),
        "the declared path must be resolved against the root directory"
    );

    let workspace = workspace_dependencies_in(&root)
        .expect("a valid root is readable")
        .expect("the root declares a workspace dependency table");

    assert!(
        workspace.contains_key("whitaker-common"),
        "the workspace table must carry the declaration it was given"
    );
}
