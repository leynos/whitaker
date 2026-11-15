//! Tests for experimental opt-in resolution.

use rstest::rstest;

use super::*;

const FAKE_EXPERIMENTAL: &[&str] = &["alpha", "beta"];
const FAKE_PROMOTED: &[&str] = &["gamma"];

#[rstest]
fn empty_request_selects_nothing() {
    let selection = resolve::<&str>(&[]).expect("an empty list is valid");
    assert_eq!(selection, ExperimentalSelection::none());
}

#[rstest]
#[case::macro_aware("macro_aware_cognitive_complexity")]
#[case::rstest_helper("rstest_helper_should_be_fixture")]
#[case::string_continuation("string_continuation_style")]
fn every_shipped_experimental_lint_resolves(#[case] name: &str) {
    let selection = resolve(&[name]).expect("shipped lint should resolve");
    assert!(selection.contains(name), "{name} should be selected");
    assert_eq!(
        selection.enabled().count(),
        1,
        "only {name} should be selected"
    );
}

#[rstest]
fn all_selects_every_shipped_experimental_lint() {
    let selected: Vec<_> = ExperimentalSelection::all().enabled().collect();
    let mut expected = EXPERIMENTAL_LINTS.to_vec();
    expected.sort_unstable();
    assert_eq!(selected, expected);
}

#[rstest]
fn duplicates_collapse() {
    let selection = resolve(&["string_continuation_style", "string_continuation_style"])
        .expect("duplicates are harmless");
    assert_eq!(selection.enabled().count(), 1);
}

#[rstest]
fn unknown_names_are_reported_together_without_duplicates() {
    let error = resolve(&[
        "typo_one",
        "string_continuation_style",
        "typo_two",
        "typo_one",
    ])
    .expect_err("unknown names must fail");
    assert_eq!(error.unknown, ["typo_one", "typo_two"]);
    assert_eq!(error.known, EXPERIMENTAL_LINTS);
}

#[rstest]
fn error_message_names_the_entries_and_the_valid_choices() {
    let message = resolve(&["typo_one"])
        .expect_err("unknown names must fail")
        .to_string();
    assert!(message.contains("typo_one"), "message was: {message}");
    assert!(
        message.contains("macro_aware_cognitive_complexity"),
        "message was: {message}"
    );
}

#[rstest]
fn names_are_matched_exactly() {
    let error = resolve(&["String_Continuation_Style"]).expect_err("case differs");
    assert_eq!(error.unknown, ["String_Continuation_Style"]);
}

#[rstest]
fn promoted_lints_are_accepted_as_redundant_and_not_selected() {
    let selection = resolve_against(
        &["alpha", "gamma", "gamma"],
        FAKE_EXPERIMENTAL,
        FAKE_PROMOTED,
    )
    .expect("promoted names stay valid");
    assert!(selection.contains("alpha"));
    assert!(
        !selection.contains("gamma"),
        "promoted lints run by default"
    );
    assert_eq!(selection.redundant(), ["gamma"]);
}

#[rstest]
fn promoted_names_appear_in_the_known_list_of_an_error() {
    let error = resolve_against(&["nope"], FAKE_EXPERIMENTAL, FAKE_PROMOTED)
        .expect_err("unknown names must fail");
    assert_eq!(error.known, ["alpha", "beta", "gamma"]);
}

#[rstest]
fn union_merges_selections_and_notes_once() {
    let first =
        resolve_against(&["alpha", "gamma"], FAKE_EXPERIMENTAL, FAKE_PROMOTED).expect("valid");
    let second =
        resolve_against(&["beta", "gamma"], FAKE_EXPERIMENTAL, FAKE_PROMOTED).expect("valid");
    let merged = first.union(&second);
    assert_eq!(merged.enabled().collect::<Vec<_>>(), ["alpha", "beta"]);
    assert_eq!(merged.redundant(), ["gamma"]);
}
