//! Tests for loading the `[whitaker]` opt-in table.

use std::convert::Infallible;

use rstest::rstest;
use whitaker_common::experimental::resolve_against;

use super::*;

/// Parse `document` as the `[whitaker]` table's contents.
fn parse(document: &str) -> Result<OptInConfig, toml::de::Error> { toml::from_str(document) }

/// An absent table selects nothing and notes nothing.
#[rstest]
fn an_absent_table_selects_nothing() {
    let loaded = load::<Infallible>(Ok(None), &ExperimentalSelection::none())
        .expect("an absent table is valid");
    assert_eq!(loaded.selection, ExperimentalSelection::none());
    assert!(loaded.redundant.is_empty());
}

/// A listed lint is selected and its neighbours are not.
#[rstest]
fn a_listed_lint_is_selected() {
    let config = parse("experimental = [\"macro_aware_cognitive_complexity\"]")
        .expect("the table should parse");
    let loaded = load::<Infallible>(Ok(Some(config)), &ExperimentalSelection::none())
        .expect("a known lint is valid");
    assert!(
        loaded
            .selection
            .contains("macro_aware_cognitive_complexity")
    );
    assert!(!loaded.selection.contains("string_continuation_style"));
}

/// A misspelt lint is an error that names it and the valid choices.
#[rstest]
fn an_unknown_lint_is_a_clear_error_naming_it() {
    let config = parse("experimental = [\"macro_aware_cognitive_complexty\"]")
        .expect("the table should parse");
    let error = load::<Infallible>(Ok(Some(config)), &ExperimentalSelection::none())
        .expect_err("a misspelt lint must not be ignored");
    let message = error.to_string();
    assert!(
        message.contains("macro_aware_cognitive_complexty"),
        "message was: {message}"
    );
    assert!(
        message.contains("known experimental lints"),
        "message was: {message}"
    );
}

/// A misspelt key in the table is rejected rather than dropped.
#[rstest]
fn an_unknown_key_in_the_table_is_rejected() {
    let error = parse("experimentl = [\"x\"]").expect_err("a misspelt key must not be dropped");
    assert!(
        error.to_string().contains("experimentl"),
        "message was: {error}"
    );
}

/// A string where a list belongs is rejected.
#[rstest]
fn a_wrongly_typed_list_is_rejected() {
    parse("experimental = \"macro_aware_cognitive_complexity\"")
        .expect_err("a string is not a list");
}

/// A failure to read the table is reported, not treated as "no opt-in".
#[rstest]
fn a_read_failure_is_reported_not_treated_as_no_opt_in() {
    let error = load(Err("permission denied"), &ExperimentalSelection::none())
        .expect_err("an unreadable table must fail");
    assert_eq!(
        error,
        OptInError::Config("permission denied".to_owned()),
        "the failure must carry the cause",
    );
}

/// A listed promoted name is noted as redundant and is not selected, because a
/// promoted lint runs by default. Stage tables are injected: no real lint has
/// been promoted yet.
#[rstest]
fn a_promoted_name_is_reported_redundant_and_not_selected() {
    let config = parse("experimental = [\"alpha\", \"gamma\"]").expect("the table should parse");
    let loaded =
        load_with::<Infallible>(Ok(Some(config)), &ExperimentalSelection::none(), |names| {
            resolve_against(names, &["alpha"], &["gamma"])
        })
        .expect("both names are known");
    assert_eq!(loaded.redundant, ["gamma"]);
    assert!(loaded.selection.contains("alpha"));
    assert!(
        !loaded.selection.contains("gamma"),
        "promoted lints run by default"
    );
}

/// The baseline is empty for the prebuilt suite and complete for a local
/// `--experimental` build.
#[rstest]
#[case::prebuilt(false, ExperimentalSelection::none())]
#[case::enable_all(true, ExperimentalSelection::all())]
fn the_baseline_follows_the_enable_all_feature(
    #[case] is_enable_all: bool,
    #[case] expected: ExperimentalSelection,
) {
    assert_eq!(baseline_for(is_enable_all), expected);
}

/// An absent table in an enable-all build still selects every experimental lint,
/// and in any other build selects none: the build feature is the whole difference.
#[rstest]
#[case::prebuilt(false, ExperimentalSelection::none())]
#[case::enable_all(true, ExperimentalSelection::all())]
fn loading_for_a_build_applies_its_baseline(
    #[case] is_enable_all: bool,
    #[case] expected: ExperimentalSelection,
) {
    let loaded =
        load_for_build::<Infallible>(Ok(None), is_enable_all).expect("an absent table is valid");
    assert_eq!(loaded.selection, expected);
}

/// The build's baseline selection survives an absent table.
#[rstest]
fn the_baseline_survives_an_empty_table() {
    let loaded = load::<Infallible>(Ok(None), &ExperimentalSelection::all())
        .expect("an absent table is valid");
    assert_eq!(loaded.selection, ExperimentalSelection::all());
}
