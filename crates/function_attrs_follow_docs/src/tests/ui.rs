//! UI regression tests for the `function_attrs_follow_docs` lint, including
//! locale-specific smoke coverage.
//!
//! Each test runs its fixtures in a child process whose `DYLINT_LOCALE` is set
//! at spawn time, so the canonical and Welsh harnesses run against the same
//! fixtures without overriding anything in the test process.

use serial_test::serial;

/// Run the canonical fixtures with no locale override.
#[test]
#[serial]
fn ui() { run_ui_with_locale("ui", "ui", None); }

/// Run the Welsh fixtures under `cy`.
#[test]
#[serial]
fn ui_runs_in_welsh_locale() { run_ui_with_locale("ui_runs_in_welsh_locale", "ui-cy", Some("cy")); }

/// Run `directory` in a child process that re-runs the test named `test_fn`.
fn run_ui_with_locale(test_fn: &str, directory: &str, locale: Option<&str>) {
    whitaker_lint_core::run_ui_tests!(directory, test = test_fn, locale = locale);
}
