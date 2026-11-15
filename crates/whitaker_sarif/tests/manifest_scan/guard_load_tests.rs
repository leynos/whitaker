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
//! about what a private helper returns.
//!
//! **Each message is captured and read rather than matched.** `#[should_panic]`
//! compares against a string fixed at compile time, so it can pin the cause
//! text but never the staged path, and the path is exactly the part of the
//! diagnostic that varies with the fixture. Capturing lets one case cover all
//! three facts at once: that the guard refused instead of certifying absence,
//! which failure it named, and which file it named.
//!
//! Fixture writes go through the shared [`Scratch`] handle; see its module
//! documentation for why staging cannot happen under `CARGO_MANIFEST_DIR`.

use std::{any::Any, panic};

use camino::Utf8Path;

use super::{assert_no_edge, scratch::scratch};

/// The dependency name every case forbids.
const FORBIDDEN: &str = "fluent-templates";

/// Opens every guard load failure, whatever the cause beneath it.
const LOAD_FAILURE: &str = "the guard cannot load a manifest";

/// Runs the guard over `path` and returns the message its panic carried.
///
/// A `panic!` rather than `expect()` on both the captured result and the
/// payload's text: this helper is called only from test bodies but is not
/// itself a `#[test]`, so `allow-expect-in-tests` does not cover it and the
/// workspace denies `expect_used` outright.
fn guard_panic_message(path: &Utf8Path) -> String {
    let payload = match panic::catch_unwind(|| assert_no_edge(path, "fixture", FORBIDDEN)) {
        Ok(()) => panic!("the guard must reject a manifest it cannot load"),
        Err(payload) => payload,
    };
    text_of(payload.as_ref())
}

/// Extracts the text carried by a panic payload.
///
/// A formatted `panic!` carries a `String` and a literal one carries a `&str`,
/// so both arms are reachable in principle. Handling only the first would turn
/// a future literal at the panic site into a confusing failure here rather than
/// a clear one.
fn text_of(payload: &(dyn Any + Send)) -> String {
    let as_string = payload.downcast_ref::<String>();
    let as_literal = payload.downcast_ref::<&str>();
    as_string.map_or_else(
        || {
            as_literal.map_or_else(
                || panic!("the guard's panic payload should carry a message"),
                |message| (*message).to_owned(),
            )
        },
        String::clone,
    )
}

#[test]
fn a_manifest_that_will_not_parse_is_reported_with_its_cause_and_path() {
    let scratch = scratch();
    scratch
        .root
        .write("Cargo.toml", "not = toml = at = all")
        .expect("the fixture manifest should be writable");
    let path = scratch.path("Cargo.toml");

    let message = guard_panic_message(&path);

    // Three facts, asserted separately because each is satisfiable by a guard
    // that is broken differently: a prefix without the cause could report every
    // failure as an unreadable file; a cause without the path would leave a
    // reader unable to find the manifest the guard refused.
    assert!(
        message.starts_with(LOAD_FAILURE),
        "the guard must refuse, not certify absence, got: {message}"
    );
    assert!(
        message.contains("not valid TOML"),
        "the parse failure must be named as a parse failure, got: {message}"
    );
    assert!(
        message.contains(path.as_str()),
        "the message must name the staged path {path}, got: {message}"
    );
}

#[test]
fn a_manifest_that_cannot_be_read_is_reported_with_its_cause_and_path() {
    // A directory where the manifest should be: located, present, and not
    // readable as a file. Reading it refuses the call, and the refusal must
    // reach the assertion rather than be swallowed.
    let scratch = scratch();
    scratch
        .root
        .create_dir("Cargo.toml")
        .expect("the fixture directory should be creatable");
    let path = scratch.path("Cargo.toml");

    let message = guard_panic_message(&path);

    assert!(
        message.starts_with(LOAD_FAILURE),
        "the guard must refuse, not certify absence, got: {message}"
    );
    assert!(
        message.contains("cannot read the manifest at"),
        "the read failure must be named as a read failure, got: {message}"
    );
    assert!(
        message.contains(path.as_str()),
        "the message must name the staged path {path}, got: {message}"
    );
}
