//! UI regression tests for the `no_std_fs_operations` lint.

use common::test_support::LocaleOverride;
use serial_test::serial;

#[test]
#[serial]
fn ui() {
    run_with_locale("ui", None);
}

#[test]
#[serial]
fn ui_runs_in_welsh() {
    run_with_locale("ui-cy", Some("cy"));
}

fn run_with_locale(directory: &str, locale: Option<&str>) {
    let _locale_guard = locale.map(LocaleOverride::set);
    whitaker::run_ui_tests!(directory).expect("UI tests should execute without diffs");
}
