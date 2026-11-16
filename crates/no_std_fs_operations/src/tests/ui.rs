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

// One locale per script family the catalogues add, so compiler output is
// checked for right-to-left, Devanagari, CJK, and Cyrillic text. The other new
// locales are covered by `diagnostics::tests`, which resolves this lint's
// messages in every shipped locale without a compiler run.
#[test]
#[serial]
fn ui_runs_in_arabic() { run_with_locale("ui-ar", Some("ar")); }

#[test]
#[serial]
fn ui_runs_in_hindi() { run_with_locale("ui-hi", Some("hi")); }

#[test]
#[serial]
fn ui_runs_in_japanese() { run_with_locale("ui-ja", Some("ja")); }

#[test]
#[serial]
fn ui_runs_in_russian() { run_with_locale("ui-ru", Some("ru")); }

#[test]
#[serial]
fn ui_runs_in_fallback_locale() { run_with_locale("ui-fallback", Some("zz")); }

fn run_with_locale(directory: &str, locale: Option<&str>) {
    with_locale(locale, || {
        whitaker_lint_core::run_ui_tests!(directory)
            .expect("UI tests should execute without diffs");
    });
}
