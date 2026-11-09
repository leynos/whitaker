//! Behavioural tests for the composed guard path.
//!
//! Two separate claims are pinned here, and they are separate because they fail
//! differently.
//!
//! The first is that the guard **rejects**. A guard is only a guard if it can
//! fail, and every test that shows it passing is worth nothing on its own: a
//! function whose body is `()` would satisfy all of them. So each rejection is
//! asserted at the same entry point the real manifest uses, which is what makes
//! the passing cases mean something.
//!
//! The second is the **non-vacuity floor**. A scan that reads no dependency
//! entry certifies nothing, so a restructure that renamed a table — or a path
//! typo that read the wrong file — would turn every guard green while examining
//! nothing. `assert_non_vacuous` is the check that forbids it, and until these
//! cases existed its two branches had no test at all: its only callers passed
//! real manifests, which always have dependencies.
//!
//! Both belong to the guard rather than to the scanner, so they live beside the
//! composed path instead of beside `dependency_tables` or `scan_for`.
//!
//! **The rejection cases are separate functions, not `#[case]` rows.** An
//! attribute written above a `#[case]` line belongs to *that* case, so a
//! `#[should_panic]` above four rows attaches to the first and silently leaves
//! the rest as tests that must *not* panic. That is not a hypothesis: it is what
//! these cases did before this file was restructured, and the three unpinned
//! rows failed with the expected message while the harness looked for a clean
//! return. Sharing the fixture text through constants keeps the functions short
//! without reintroducing the hazard.

use rstest::rstest;

use super::{
    ScanOutcome,
    assert_no_edge_in,
    assert_non_vacuous,
    parse_manifest,
    scan_for,
    workspace_table_of,
};

/// The dependency name every fixture forbids.
const FORBIDDEN: &str = "fluent-templates";

// -- The non-vacuity floor rejects what it must -----------------------------

#[rstest]
#[should_panic(expected = "declares no dependency table; the guard is vacuous")]
fn a_manifest_with_no_dependency_table_fails_the_guard() {
    // A manifest that parses and declares nothing at all. Its only failure is
    // that the guard would certify absence from zero evidence.
    assert_non_vacuous(
        &parse_manifest(concat!(
            "[package]\n",
            "name = \"whitaker_brain_trust_sarif\"\n",
            "version = \"0.1.0\"\n",
        )),
        "fixture",
    );
}

#[rstest]
#[should_panic(expected = "declares no dependency; the guard is vacuous")]
fn an_empty_dependency_table_fails_the_guard() {
    // The distinction the two messages draw is the point: a table *was* found,
    // so the first check passed, and the second is what catches this manifest.
    assert_non_vacuous(&parse_manifest("[dependencies]\n"), "fixture");
}

#[rstest]
#[should_panic(expected = "the guard is vacuous")]
fn the_guard_itself_rejects_a_vacuous_manifest() {
    // The two cases above show the floor *works*. They do not show the guard
    // *calls* it, and the difference is not academic: deleting the
    // `assert_non_vacuous` call from the funnel left all 46 tests in this
    // target passing, because every other fixture declares a dependency and so
    // never reaches the floor. This case routes a vacuous manifest through the
    // funnel, so the call site itself is pinned.
    run_guard("[dependencies]\n", None);
}

#[rstest]
fn a_manifest_with_a_dependency_passes_the_floor() {
    // The positive case, without which the two rejections above could be
    // satisfied by a function that panics unconditionally.
    assert_non_vacuous(
        &parse_manifest("[dependencies]\nserde = \"1\"\n"),
        "fixture",
    );
}

// -- The guard rejects a forbidden edge, in every shape ---------------------

/// Runs a fixture document and its literal workspace through the guard.
///
/// This is the same funnel the real manifests use: non-vacuity, then scan, then
/// require certified absence. Taking it here rather than calling `scan_for`
/// directly is what keeps the fixture cases honest — a fixture that bypassed the
/// floor could not notice if the floor stopped running.
fn run_guard(manifest: &str, workspace: Option<&str>) {
    let document = parse_manifest(manifest);
    let root = workspace.map(parse_manifest);
    let dependencies = root.as_ref().and_then(workspace_table_of);
    assert_no_edge_in(&document, "fixture", FORBIDDEN, dependencies.as_ref());
}

#[rstest]
#[should_panic(expected = "must not depend on fluent-templates")]
fn the_guard_rejects_a_direct_edge() {
    run_guard("[dependencies]\nfluent-templates = \"0.15\"\n", None);
}

#[rstest]
#[should_panic(expected = "must not depend on fluent-templates")]
fn the_guard_rejects_an_edge_hidden_behind_a_rename() {
    // A key-only scan passes this manifest. The rejection is the proof that the
    // guard resolves the `package` field rather than the key.
    run_guard(
        "[dependencies]\nloc = { package = \"fluent-templates\" }\n",
        None,
    );
}

#[rstest]
#[should_panic(expected = "must not depend on fluent-templates")]
fn the_guard_rejects_an_edge_behind_a_target_selector() {
    run_guard(
        "[target.'cfg(unix)'.dependencies]\nfluent-templates = \"0.15\"\n",
        None,
    );
}

#[rstest]
#[should_panic(expected = "must not depend on fluent-templates")]
fn the_guard_rejects_an_edge_inherited_through_the_workspace() {
    // The member names no forbidden package; only the root declaration does.
    // Without the workspace table in the funnel this manifest reads as clean.
    run_guard(
        "[dependencies]\nloc = { workspace = true }\n",
        Some("[workspace.dependencies]\nloc = { package = \"fluent-templates\" }\n"),
    );
}

#[rstest]
#[should_panic(expected = "must not depend on fluent-templates")]
fn the_guard_rejects_an_unresolvable_entry() {
    // `loc` might be the forbidden package under a name the scan cannot read,
    // so the guard must not certify absence. This is the fail-closed rule as a
    // *guard* claim rather than a scanner claim: `is_found()` alone would pass
    // this manifest.
    run_guard("[dependencies]\nloc = { workspace = true }\n", None);
}

#[rstest]
fn the_guard_accepts_a_clean_manifest() {
    // Without this the rejections above are satisfied by a function that panics
    // on every input, which would guard nothing.
    run_guard(
        "[dependencies]\nwhitaker-common = { workspace = true }\n",
        Some("[workspace.dependencies]\nwhitaker-common = { path = \"common\" }\n"),
    );
}

#[rstest]
fn the_guard_reads_a_declared_entry_beside_an_unreadable_one() {
    // An explicit declaration outranks an unresolved entry, so a manifest that
    // both declares the forbidden package and carries an unreadable entry is a
    // finding, not a failure to resolve. Asserted through the scanner because
    // the guard's own message cannot distinguish the two — both panic.
    let document = parse_manifest(concat!(
        "[dependencies]\n",
        "aaa = { workspace = true }\n",
        "fluent-templates = \"0.15\"\n",
    ));
    assert_eq!(
        scan_for(&document, FORBIDDEN, None),
        ScanOutcome::Found("dependencies.fluent-templates".to_owned()),
        "a declared edge must be named even when another entry is unreadable"
    );
}
