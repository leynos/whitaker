//! UI regression tests for the `module_must_have_inner_docs` lint across
//! English and Welsh locales.

use common::test_support::LocaleOverride;
use rstest::rstest;
use serial_test::serial;

/// Runs UI regression tests for the `module_must_have_inner_docs` lint under
/// different locale configurations, verifying that diagnostics render correctly.
///
/// # Cases
///
/// - `default_locale`: Uses the default English locale (`"ui"` fixtures, `None`).
/// - `welsh_locale`: Uses Welsh localisation (`"ui-cy"` fixtures, `Some("cy")`).
/// - `unsupported_locale_falls_back_to_english`: Uses an unsupported locale
///   (`"xx-YY"`), expecting fallback to English (`"ui"` fixtures).
///
/// # Example
///
/// ```ignore
/// // Default locale case: matches "ui" baselines with no locale override.
/// ui_tests_across_locales("ui", None);
/// ```
#[rstest]
#[case::default_locale("ui", None)]
#[case::welsh_locale("ui-cy", Some("cy"))]
#[case::unsupported_locale_falls_back_to_english("ui", Some("xx-YY"))]
#[serial]
fn ui_tests_across_locales(#[case] directory: &str, #[case] locale: Option<&str>) {
    let _guard = locale.map(LocaleOverride::set);
    whitaker::run_ui_tests!(directory).expect("UI tests should execute without diffs");
}
