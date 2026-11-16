//! Tests for experimental opt-in resolution.

use std::collections::BTreeSet;

use proptest::prelude::*;
use rstest::rstest;

use super::*;

const FAKE_EXPERIMENTAL: &[&str] = &["alpha", "beta"];
const FAKE_PROMOTED: &[&str] = &["gamma"];

/// Generate a requested name: experimental, promoted, or unknown.
fn any_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("alpha"),
        Just("beta"),
        Just("gamma"),
        Just("typo_one"),
        Just("typo_two"),
    ]
    .prop_map(str::to_owned)
}

/// Generate a name that is known, either experimental or promoted.
fn known_name() -> impl Strategy<Value = String> {
    prop_oneof![Just("alpha"), Just("beta"), Just("gamma")].prop_map(str::to_owned)
}

/// The distinct entries of `names` that satisfy `keep`, in first-occurrence order.
fn distinct_in_order(names: &[String], keep: impl Fn(&str) -> bool) -> Vec<String> {
    let mut seen = Vec::new();
    for name in names.iter().filter(|name| keep(name.as_str())) {
        if !seen.contains(name) {
            seen.push(name.clone());
        }
    }
    seen
}

proptest! {
    /// Resolution succeeds exactly when no name is unknown; on success every
    /// experimental name is selected and each promoted name is noted once, in
    /// request order; on failure the error carries the distinct unknown names
    /// in request order.
    #[test]
    fn resolution_agrees_with_its_specification(
        names in prop::collection::vec(any_name(), 0..=24_usize)
    ) {
        let is_known = |name: &str| ["alpha", "beta", "gamma"].contains(&name);
        let unknown = distinct_in_order(&names, |name| !is_known(name));
        let result = resolve_against(&names, FAKE_EXPERIMENTAL, FAKE_PROMOTED);

        match result {
            Err(error) => {
                prop_assert!(!unknown.is_empty(), "failed although every name is known");
                prop_assert_eq!(error.unknown, unknown);
            }
            Ok(selection) => {
                prop_assert!(unknown.is_empty(), "succeeded despite unknown names");
                for name in FAKE_EXPERIMENTAL {
                    prop_assert_eq!(
                        selection.contains(name),
                        names.iter().any(|requested| requested == name)
                    );
                }
                let promoted = distinct_in_order(&names, |name| FAKE_PROMOTED.contains(&name));
                prop_assert_eq!(selection.redundant(), promoted.as_slice());
            }
        }
    }

    /// Union is the set union of the enabled lints, never repeats a redundant
    /// note, and is idempotent.
    #[test]
    fn union_is_a_set_union_without_repeated_notes(
        left in prop::collection::vec(known_name(), 0..=12_usize),
        right in prop::collection::vec(known_name(), 0..=12_usize),
    ) {
        let first = resolve_against(&left, FAKE_EXPERIMENTAL, FAKE_PROMOTED)
            .expect("known names resolve");
        let second = resolve_against(&right, FAKE_EXPERIMENTAL, FAKE_PROMOTED)
            .expect("known names resolve");
        let merged = first.clone().union(&second);

        let expected: BTreeSet<&str> = first.enabled().chain(second.enabled()).collect();
        prop_assert_eq!(merged.enabled().collect::<BTreeSet<_>>(), expected);
        let notes: BTreeSet<&str> = merged.redundant().iter().copied().collect();
        prop_assert_eq!(notes.len(), merged.redundant().len(), "a redundant note repeats");
        prop_assert_eq!(first.clone().union(&first), first);
    }
}

/// An empty request selects nothing.
#[rstest]
fn empty_request_selects_nothing() {
    let selection = resolve::<&str>(&[]).expect("an empty list is valid");
    assert_eq!(selection, ExperimentalSelection::none());
}

/// Each shipped experimental lint resolves on its own.
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

/// `all` selects every shipped experimental lint.
#[rstest]
fn all_selects_every_shipped_experimental_lint() {
    let selected: Vec<_> = ExperimentalSelection::all().enabled().collect();
    let mut expected = EXPERIMENTAL_LINTS.to_vec();
    expected.sort_unstable();
    assert_eq!(selected, expected);
}

/// Listing a lint twice selects it once.
#[rstest]
fn duplicates_collapse() {
    let selection = resolve(&["string_continuation_style", "string_continuation_style"])
        .expect("duplicates are harmless");
    assert_eq!(selection.enabled().count(), 1);
}

/// Every unknown name is reported at once, each once.
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

/// The error message names the bad entry and the valid choices.
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

/// Names match exactly, so a different case is unknown.
#[rstest]
fn names_are_matched_exactly() {
    let error = resolve(&["String_Continuation_Style"]).expect_err("case differs");
    assert_eq!(error.unknown, ["String_Continuation_Style"]);
}

/// A promoted lint is accepted, noted as redundant, and not selected.
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

/// An error lists promoted names among the ones a caller could have used.
#[rstest]
fn promoted_names_appear_in_the_known_list_of_an_error() {
    let error = resolve_against(&["nope"], FAKE_EXPERIMENTAL, FAKE_PROMOTED)
        .expect_err("unknown names must fail");
    assert_eq!(error.known, ["alpha", "beta", "gamma"]);
}

/// Merging selections unions the lints and notes each redundant name once.
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
