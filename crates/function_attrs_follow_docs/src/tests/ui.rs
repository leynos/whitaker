//! UI regression tests for the `function_attrs_follow_docs` lint, including
//! locale-specific smoke coverage.
//!
//! The tests serialise execution so temporary `DYLINT_LOCALE` overrides remain
//! race-free while the canonical and Welsh harnesses run against the same
//! fixtures.

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
