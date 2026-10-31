//! Unit tests for validated settings conversion.

use super::{is_valid_window, normalize, normalize_provider_attributes, validate_output};
use crate::settings::Configuration;

/// Normalize invalid bumpy-road tuning to the lint's documented defaults.
#[test]
fn normalizes_bumpy_road_tuning() {
    let mut configuration = Configuration::default();
    configuration.bumpy_road_function.threshold = -1.0;
    configuration.bumpy_road_function.window = 2;
    configuration.bumpy_road_function.min_bump_lines = 0;
    configuration.bumpy_road_function.weights.depth = -1.0;
    configuration.bumpy_road_function.weights.flow = f64::NAN;
    let settings = normalize(configuration);
    let bumpy = &settings.bumpy_road_function;
    assert_eq!(bumpy.threshold, 2.5);
    assert_eq!(bumpy.window, 3);
    assert_eq!(bumpy.min_bump_lines, 1);
    assert_eq!(bumpy.weights.depth, 1.0);
    assert_eq!(bumpy.weights.flow, 0.5);
}

/// Clamp branch and fixture thresholds with the domain limits.
#[test]
fn normalizes_branch_and_fixture_thresholds() {
    let mut configuration = Configuration::default();
    configuration.conditional_max_n_branches.max_branches = 0;
    configuration.rstest_helper_should_be_fixture.min_calls = 0;
    configuration
        .rstest_helper_should_be_fixture
        .min_distinct_tests = 1;
    let settings = normalize(configuration);
    assert_eq!(settings.conditional_max_n_branches.max_branches, 1);
    assert_eq!(settings.rstest_helper_should_be_fixture.min_calls, 2);
    assert_eq!(
        settings.rstest_helper_should_be_fixture.min_distinct_tests,
        2
    );
}

/// Reject a progress policy outside the documented set.
#[test]
fn rejects_unknown_progress_policy() {
    let mut configuration = Configuration::default();
    configuration.output.progress = String::from("fancy");
    assert!(validate_output(&configuration.output).is_err());
}

/// Accept only non-zero odd smoothing windows.
#[rstest::rstest]
#[case(0, false)]
#[case(1, true)]
#[case(2, false)]
#[case(3, true)]
#[case(9, true)]
fn recognizes_valid_windows(#[case] window: usize, #[case] expected: bool) {
    assert_eq!(is_valid_window(window), expected);
}

/// Canonicalize provider attribute names and restore defaults when empty.
#[rstest::rstest]
#[case(vec![" rstest::case ".to_owned(), "case".to_owned()], vec!["case".to_owned()])]
#[case(vec![String::new(), " values ".to_owned()], vec!["values".to_owned()])]
#[case(
    Vec::new(),
    vec![
        "case".to_owned(),
        "values".to_owned(),
        "files".to_owned(),
        "future".to_owned(),
        "context".to_owned(),
    ]
)]
fn canonicalizes_provider_attributes(#[case] input: Vec<String>, #[case] expected: Vec<String>) {
    assert_eq!(normalize_provider_attributes(input), expected);
}
