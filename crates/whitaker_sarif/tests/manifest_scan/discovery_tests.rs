//! Unit tests for manifest discovery's three outcomes.
//!
//! Discovery used to answer a question with two possible replies — "here is the
//! manifest" and "there is none" — when there are three. The missing one was "I
//! could not tell": `is_file()` returns `false` for a permission denial exactly
//! as it does for a missing file, so a guard that was refused permission
//! reported the same clean result as one that looked and found nothing.
//!
//! These tests exercise [`probe`], the function that draws the distinction, in
//! isolation. Testing it through `manifest_if_present` would mean staging a
//! permission-denied directory somewhere under `CARGO_MANIFEST_DIR`, which this
//! crate does not control; a temp directory is the only honest way to produce a
//! refused lookup. The composition itself is covered by
//! `architecture_boundary.rs`, which runs real crates through
//! `manifest_if_present`.
//!
//! Fixture writes go through a capability-scoped `cap_std` handle for the same
//! reason the rest of this toolkit does: the workspace denies ambient
//! `std::fs`, and a test is not an exemption.
//!
//! **The fixture stages itself inside each test rather than in a `#[fixture]`.**
//! The workspace denies `expect_used` and sets `allow-expect-in-tests = true`,
//! which covers a `#[test]` body but *not* a fixture helper — the helper is
//! neither `#[test]` nor under `#[cfg(test)]`. A fallible fixture cannot be used
//! here either, because the rejection tests are `#[should_panic]` and the
//! workspace denies `panic_in_result_fn`, so they may not return `Result`. The
//! staging therefore happens in the test, where `expect()` is permitted and the
//! panic it produces is the test's own failure.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

use super::{DiscoveryError, probe, resolve_declared};

/// A staged temporary directory: its handle, its UTF-8 root, and a keepalive.
///
/// The `TempDir` field is never read. It exists so the directory is removed
/// when the value drops, which is the whole of its job; the underscore says so.
struct Scratch {
    _directory: TempDir,
    root: Dir,
    root_path: Utf8PathBuf,
}

/// Creates a scratch directory, panicking when the host cannot provide one.
///
/// Panicking is the same outcome `expect()` in a test body would produce, and
/// this is called only from test bodies, where the workspace permits it.
fn scratch() -> Scratch {
    let directory = match TempDir::new() {
        Ok(directory) => directory,
        Err(error) => panic!("a temporary directory should be creatable: {error}"),
    };
    // Let-else rather than a `match` that yields the path: the workspace also
    // denies `option_if_let_else`, and a two-arm `match` on an `Option` that
    // produces a value is the shape it flags.
    let Some(root_path) = Utf8Path::from_path(directory.path()).map(Utf8Path::to_owned) else {
        panic!("the temporary directory path should be valid UTF-8");
    };
    let root = match Dir::open_ambient_dir(&root_path, ambient_authority()) {
        Ok(root) => root,
        Err(error) => panic!("the temporary directory should open: {error}"),
    };
    Scratch {
        _directory: directory,
        root,
        root_path,
    }
}

/// Joins `name` onto the scratch root.
fn at(scratch: &Scratch, name: &str) -> Utf8PathBuf { scratch.root_path.join(name) }

#[test]
fn a_present_manifest_is_reported_present() {
    let scratch = scratch();
    scratch
        .root
        .write("Cargo.toml", "[package]\n")
        .expect("the fixture manifest should be writable");

    assert!(
        probe(&at(&scratch, "Cargo.toml")).expect("the lookup should succeed"),
        "a manifest that exists must be reported present"
    );
}

#[test]
fn a_missing_manifest_is_reported_absent() {
    // `NotFound` is the one failure that proves absence rather than merely
    // failing to look, which is what makes `Ok(false)` a safe answer here.
    let scratch = scratch();
    assert!(
        !probe(&at(&scratch, "Cargo.toml")).expect("the lookup should succeed"),
        "a manifest that does not exist must be reported absent, not as an error"
    );
}

#[test]
fn a_directory_in_place_of_a_manifest_is_an_error() {
    // Present but not a file is not absence: the path exists, so a caller
    // reading `Ok(false)` would conclude the crate is missing when its
    // manifest path is occupied by something else.
    let scratch = scratch();
    scratch
        .root
        .create_dir("Cargo.toml")
        .expect("the fixture directory should be creatable");

    let error = probe(&at(&scratch, "Cargo.toml")).expect_err("a directory is not a manifest");
    assert!(
        matches!(error, DiscoveryError::NotAFile { .. }),
        "a directory at the manifest path must be reported as such, got {error}"
    );
}

#[test]
fn a_parent_that_is_a_file_makes_the_child_absent() {
    // `Cargo.toml/Cargo.toml` cannot exist because `Cargo.toml` is a regular
    // file. The kernel reports `NotADirectory`, and reading that as a refusal
    // to look would turn a proven absence into a guard failure. The convention
    // fallback hits this shape whenever a crate name matches a file.
    let scratch = scratch();
    scratch
        .root
        .write("Cargo.toml", "[package]\n")
        .expect("the fixture manifest should be writable");

    assert!(
        !probe(&at(&scratch, "Cargo.toml").join("Cargo.toml")).expect("the lookup should succeed"),
        "a path below a regular file cannot exist and must be reported absent"
    );
}

#[test]
#[cfg(unix)]
fn a_refused_lookup_is_an_error_not_an_absence() {
    // The defect this whole file exists for. The directory is made
    // *unsearchable* — mode `0o600`, so it still opens as a handle but nothing
    // inside it can be named. That is what produces a refusal rather than an
    // absence, and it is deliberately not `chmod 000` on the directory:
    // `metadata` on a directory whose own mode forbids access still succeeds,
    // because the call describes the directory itself rather than looking
    // inside it. Mutating the error arm to return `Ok(false)` left every test
    // passing until this fixture was corrected, which is how that distinction
    // was found.
    use std::io;

    use cap_std::fs::PermissionsExt;

    let scratch = scratch();
    scratch
        .root
        .create_dir("unsearchable")
        .expect("the fixture directory should be creatable");
    scratch
        .root
        .set_permissions("unsearchable", cap_std::fs::Permissions::from_mode(0o600))
        .expect("the fixture permissions should be settable");
    let locked = at(&scratch, "unsearchable").join("Cargo.toml");

    // The precondition is measured with a raw `cap_std` lookup, deliberately
    // *not* through `probe`. Measuring it through the code under test makes the
    // case self-defeating: folding the refusal into `Ok(false)` would make the
    // precondition read false, the test skip, and the suite stay green — which
    // is exactly what happened before this was changed. A distinct call path
    // cannot be mutated by the same edit.
    //
    // Root, and any host with `CAP_DAC_OVERRIDE`, bypasses the mode bits; the
    // case is skipped there rather than reported as passing.
    let refused = raw_lookup_error(&locked)
        .is_some_and(|error| error.kind() == io::ErrorKind::PermissionDenied);
    if !refused {
        return;
    }

    let error = probe(&locked).expect_err("a refused lookup must not read as absence");
    assert!(
        matches!(error, DiscoveryError::Unreadable { .. }),
        "a refused lookup must be reported as unreadable, got {error}"
    );
}

/// Performs the same filesystem lookup `probe` performs, without `probe`.
///
/// This exists so one test can establish its precondition independently of the
/// behaviour that test is checking. It is the only reason it is not just a
/// second `probe` call.
#[cfg(unix)]
fn raw_lookup_error(path: &Utf8Path) -> Option<std::io::Error> {
    let directory = path.parent()?;
    let filename = path.file_name()?;
    let handle = Dir::open_ambient_dir(directory, ambient_authority()).ok()?;
    handle.metadata(filename).err()
}

/// A declaration that points at nothing while the sibling holds the manifest is
/// a conflict, not an absence.
///
/// Both single-source answers are wrong here. Trusting the declaration reports
/// the crate absent, so the mapping-crate guard skips a manifest that exists —
/// the silent dormancy this whole file exists to prevent. Trusting the sibling
/// accepts a root manifest that names the wrong path and never reports it. The
/// conflict is the honest answer, and it is reachable because the two sources
/// are read independently.
#[test]
fn a_declared_path_with_no_manifest_beside_a_present_sibling_is_a_conflict() {
    let scratch = scratch();
    scratch
        .root
        .create_dir_all("declared_elsewhere")
        .expect("the declared directory should be creatable");
    scratch
        .root
        .create_dir_all("crates/present_crate")
        .expect("the sibling directory should be creatable");
    scratch
        .root
        .write("crates/present_crate/Cargo.toml", "[package]\n")
        .expect("the sibling manifest should be writable");

    let error = resolve_declared(
        "present_crate",
        at(&scratch, "declared_elsewhere").join("Cargo.toml"),
        Some(&at(&scratch, "crates/present_crate").join("Cargo.toml")),
    )
    .expect_err("a stale declaration beside a present sibling must be reported");

    // The message names all three facts, because a diagnostic that omits the
    // sibling path leaves the reader unable to see that the crate is present.
    let rendered = error.to_string();
    for expected in ["declared_elsewhere", "present_crate", "stale"] {
        assert!(
            rendered.contains(expected),
            "the conflict should name {expected:?}, got: {rendered}"
        );
    }
}

/// A declaration that resolves is returned without consulting the sibling.
///
/// The declaration is authoritative when it is right, so a sibling that also
/// happens to exist must not turn a healthy workspace into a conflict.
#[test]
fn a_present_declaration_wins_over_an_absent_sibling() {
    let scratch = scratch();
    scratch
        .root
        .create_dir_all("declared_here")
        .expect("the declared directory should be creatable");
    scratch
        .root
        .write("declared_here/Cargo.toml", "[package]\n")
        .expect("the declared manifest should be writable");

    let found = resolve_declared(
        "declared_crate",
        at(&scratch, "declared_here").join("Cargo.toml"),
        None,
    )
    .expect("a declaration naming a real manifest is decidable");

    assert_eq!(
        found.as_deref(),
        Some(at(&scratch, "declared_here").join("Cargo.toml").as_path()),
        "a present declaration must be returned as-is"
    );
}

/// A declaration that resolves to nothing, with no sibling either, is absent.
///
/// This is the ordinary case for a crate that simply is not in the workspace,
/// and it must stay `Ok(None)` rather than becoming a conflict: there is
/// nothing to disagree about.
#[test]
fn a_declaration_with_neither_manifest_nor_sibling_is_absent() {
    let scratch = scratch();
    scratch
        .root
        .create_dir_all("declared_elsewhere")
        .expect("the declared directory should be creatable");

    let found = resolve_declared(
        "absent_crate",
        at(&scratch, "declared_elsewhere").join("Cargo.toml"),
        Some(&at(&scratch, "crates/absent_crate").join("Cargo.toml")),
    )
    .expect("two confirmed absences are decidable");

    assert!(
        found.is_none(),
        "a crate present at neither candidate is absent, got {found:?}"
    );
}
