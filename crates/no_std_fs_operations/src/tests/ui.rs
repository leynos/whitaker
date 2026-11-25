//! UI regression tests for the `no_std_fs_operations` lint.
//!
//! Each test runs its fixtures in a child process whose `DYLINT_LOCALE` is set
//! at spawn time, so no test overrides anything in the test process.

use serial_test::serial;

/// Run the canonical fixtures with no locale override.
#[test]
#[serial]
fn ui() { run_with_locale("ui", "ui", None); }

/// Run the Welsh fixtures under `cy`.
#[test]
#[serial]
fn ui_runs_in_welsh() { run_with_locale("ui_runs_in_welsh", "ui-cy", Some("cy")); }

/// Run the Gaelic fixtures under `gd`.
#[test]
#[serial]
fn ui_runs_in_gaelic() { run_with_locale("ui_runs_in_gaelic", "ui-gd", Some("gd")); }

// One locale per script family the catalogues add, so compiler output is
// checked for right-to-left, Devanagari, CJK, and Cyrillic text. The other new
// locales are covered by `diagnostics::tests`, which resolves this lint's
// messages in every shipped locale without a compiler run.

/// Run the right-to-left fixtures under `ar`.
#[test]
#[serial]
fn ui_runs_in_arabic() { run_with_locale("ui_runs_in_arabic", "ui-ar", Some("ar")); }

/// Run the Devanagari fixtures under `hi`.
#[test]
#[serial]
fn ui_runs_in_hindi() { run_with_locale("ui_runs_in_hindi", "ui-hi", Some("hi")); }

/// Run the CJK fixtures under `ja`.
#[test]
#[serial]
fn ui_runs_in_japanese() { run_with_locale("ui_runs_in_japanese", "ui-ja", Some("ja")); }

/// Run the Cyrillic fixtures under `ru`.
#[test]
#[serial]
fn ui_runs_in_russian() { run_with_locale("ui_runs_in_russian", "ui-ru", Some("ru")); }

/// Run the fallback fixtures under an unsupported locale.
#[test]
#[serial]
fn ui_runs_in_fallback_locale() {
    run_with_locale("ui_runs_in_fallback_locale", "ui-fallback", Some("zz"));
}

/// Run `directory` in a child process that re-runs the test named `test_fn`.
fn run_with_locale(test_fn: &str, directory: &str, locale: Option<&str>) {
    whitaker_lint_core::run_ui_tests!(directory, test = test_fn, locale = locale);
}
