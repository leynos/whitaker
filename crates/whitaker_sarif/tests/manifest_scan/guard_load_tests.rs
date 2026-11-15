//! Behavioural tests for the guard's manifest-load failure branch.
//!
//! [`assert_no_edge`] reads a file, and reading a file can fail. When it does,
//! the guard must fail the assertion with the typed error's text rather than
//! return a `Result` a caller could ignore, or pass the manifest as if it had
//! been examined. `load_or_fail` is the branch that makes that true, and until
//! these cases existed every file-backed test staged *valid* TOML — so the
//! error branch had no test at all, and a `load_or_fail` that returned an empty
//! table on failure would have turned every unreadable manifest into a vacuous
//! pass with the whole suite still green.
//!
//! The cases are stated through the public entry point rather than through
//! `load_or_fail` directly, because the claim is about what the guard does, not
//! about what a private helper returns. That is also why the expected message
//! names the load failure: a guard that failed for some unrelated reason — a
//! vacuity trip, say — would not produce it.
//!
//! Fixture writes go through the shared [`Scratch`] handle; see its module
//! documentation for why staging cannot happen under `CARGO_MANIFEST_DIR`.

use super::{
    assert_no_edge,
    scratch::{Scratch, scratch},
};

/// The dependency name every case forbids.
const FORBIDDEN: &str = "fluent-templates";

/// Stages a manifest path whose contents are not readable TOML, keeping the
/// scratch root alive beside it.
///
/// A plain `panic!` rather than `expect()`: this helper is called only from
/// test bodies, but it is not itself a `#[test]`, so `allow-expect-in-tests`
/// does not cover it and the workspace denies `expect_used` outright.
fn staged_directory() -> (Scratch, camino::Utf8PathBuf) {
    let scratch = scratch();
    if let Err(error) = scratch.root.create_dir("Cargo.toml") {
        panic!("the fixture directory should be creatable: {error}");
    }
    let path = scratch.path("Cargo.toml");
    (scratch, path)
}

#[test]
#[should_panic(expected = "the guard cannot load a manifest")]
fn the_file_backed_guard_rejects_a_manifest_that_will_not_parse() {
    // A member manifest that is located, present, and not TOML. The load fails,
    // and the guard must panic rather than certify absence from an empty table.
    let scratch = scratch();
    scratch
        .root
        .write("Cargo.toml", "[dependencies\nnot toml")
        .expect("the fixture manifest should be writable");

    assert_no_edge(&scratch.path("Cargo.toml"), "fixture", FORBIDDEN);
}

#[test]
#[should_panic(expected = "not valid TOML")]
fn the_parse_failure_is_named_as_a_parse_failure() {
    // The message must say *which* failure it was. A guard that reported a
    // parse failure as an unreadable file would still panic, so the loose
    // prefix above is not enough on its own; naming the cause pins it.
    let scratch = scratch();
    scratch
        .root
        .write("Cargo.toml", "not = toml = at = all")
        .expect("the fixture manifest should be writable");

    assert_no_edge(&scratch.path("Cargo.toml"), "fixture", FORBIDDEN);
}

#[test]
#[should_panic(expected = "the guard cannot load a manifest")]
fn the_file_backed_guard_rejects_an_unreadable_manifest() {
    // A directory where the manifest should be. Reading it refuses the call,
    // and the refusal must reach the assertion rather than being swallowed.
    let (_scratch, path) = staged_directory();

    assert_no_edge(&path, "fixture", FORBIDDEN);
}

#[test]
#[should_panic(expected = "cannot read the manifest at")]
fn the_read_failure_names_the_path() {
    // The typed error's text carries the path, and the guard must not replace
    // it with a generic message — the path is the whole diagnostic when the
    // guard is run over a workspace nobody has memorized.
    let (_scratch, path) = staged_directory();

    assert_no_edge(&path, "fixture", FORBIDDEN);
}
