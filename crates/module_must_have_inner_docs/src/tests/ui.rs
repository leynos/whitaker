//! UI regression tests for the `module_must_have_inner_docs` lint across
//! English and Welsh locales.
//!
//! Each test runs its fixtures in a child process whose `DYLINT_LOCALE` is set
//! at spawn time, so no test overrides anything in the test process.

use serial_test::serial;

/// Run the English fixtures with no locale override.
#[test]
#[serial]
fn default_locale() { run_ui_across_locales("default_locale", "ui", None); }

/// Run the Welsh fixtures under `cy`.
#[test]
#[serial]
fn welsh_locale() { run_ui_across_locales("welsh_locale", "ui-cy", Some("cy")); }

/// Run the English fixtures under an unsupported locale, which falls back to
/// English.
#[test]
#[serial]
fn unsupported_locale_falls_back_to_english() {
    run_ui_across_locales(
        "unsupported_locale_falls_back_to_english",
        "ui",
        Some("xx-YY"),
    );
}

/// Run `directory` in a child process that re-runs the test named `test_fn`,
/// verifying that diagnostics render correctly under `locale`.
fn run_ui_across_locales(test_fn: &str, directory: &str, locale: Option<&str>) {
    whitaker_lint_core::run_ui_tests!(directory, test = test_fn, locale = locale);
}
