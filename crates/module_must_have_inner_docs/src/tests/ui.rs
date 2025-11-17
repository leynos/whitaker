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

fn run_ui_with_locale(directory: &str, locale: Option<&str>) {
    let _guard = locale.map(LocaleOverride::set);
    whitaker::run_ui_tests!(directory).expect("UI tests should execute without diffs");
}
