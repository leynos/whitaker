//! UI regression tests for the `no_std_fs_operations` lint.

use serial_test::serial;
use whitaker_common::test_support::with_locale;

#[test]
#[serial]
fn ui() { run_with_locale("ui", None); }

#[test]
#[serial]
fn ui_runs_in_welsh() { run_with_locale("ui-cy", Some("cy")); }

#[test]
#[serial]
fn ui_runs_in_gaelic() { run_with_locale("ui-gd", Some("gd")); }

#[test]
#[serial]
fn ui_runs_in_fallback_locale() { run_with_locale("ui-fallback", Some("zz")); }

fn run_with_locale(directory: &str, locale: Option<&str>) {
    with_locale(locale, || {
        whitaker::run_ui_tests!(directory).expect("UI tests should execute without diffs");
    });
}
