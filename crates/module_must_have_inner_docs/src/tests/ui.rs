//! UI regression tests for the `module_must_have_inner_docs` lint across
//! English and Welsh locales.

use common::test_support::LocaleOverride;
use serial_test::serial;

#[test]
#[serial]
fn ui() {
    run_ui_with_locale("ui", None);
}

#[test]
#[serial]
fn ui_runs_in_welsh_locale() {
    run_ui_with_locale("ui-cy", Some("cy"));
}

#[test]
#[serial]
fn ui_falls_back_to_english_for_unsupported_locale() {
    // Use an obviously unsupported locale; diagnostics should still render,
    // implicitly falling back to English and matching the "ui" baselines.
    run_ui_with_locale("ui", Some("xx-YY"));
}

fn run_ui_with_locale(directory: &str, locale: Option<&str>) {
    let _guard = locale.map(LocaleOverride::set);
    whitaker::run_ui_tests!(directory).expect("UI tests should execute without diffs");
}
