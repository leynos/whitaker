//! Tests for loading the `[whitaker]` opt-in table.

use std::convert::Infallible;

use rstest::rstest;

use super::*;

fn parse(document: &str) -> Result<OptInConfig, toml::de::Error> { toml::from_str(document) }

#[rstest]
fn an_absent_table_selects_nothing() {
    let loaded = load::<Infallible>(Ok(None), &ExperimentalSelection::none())
        .expect("an absent table is valid");
    assert_eq!(loaded.selection, ExperimentalSelection::none());
    assert!(loaded.redundant.is_empty());
}

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

#[rstest]
fn an_unknown_key_in_the_table_is_rejected() {
    let error = parse("experimentl = [\"x\"]").expect_err("a misspelt key must not be dropped");
    assert!(
        error.to_string().contains("experimentl"),
        "message was: {error}"
    );
}

#[rstest]
fn a_wrongly_typed_list_is_rejected() {
    parse("experimental = \"macro_aware_cognitive_complexity\"")
        .expect_err("a string is not a list");
}

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

#[rstest]
fn the_baseline_survives_an_empty_table() {
    let loaded = load::<Infallible>(Ok(None), &ExperimentalSelection::all())
        .expect("an absent table is valid");
    assert_eq!(loaded.selection, ExperimentalSelection::all());
}
