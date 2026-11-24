//! UI regression tests for the `function_attrs_follow_docs` lint, including
//! locale-specific smoke coverage.
//!
//! The locale reaches the compiler through the harness rather than through the
//! test process's environment, so the canonical and Welsh harnesses can run
//! against the same fixtures without overriding anything global.

use serial_test::serial;

#[test]
#[serial]
fn ui() { run_ui_with_locale("ui", None); }

#[test]
#[serial]
fn ui_runs_in_welsh_locale() { run_ui_with_locale("ui-cy", Some("cy")); }

fn run_ui_with_locale(directory: &str, locale: Option<&str>) {
    whitaker_lint_core::run_ui_tests!(directory, locale = locale)
        .expect("UI tests should execute without diffs");
}
