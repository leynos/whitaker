//! Unit tests for validated settings conversion.

use proptest::prelude::*;

use super::{is_valid_window, normalize, normalize_provider_attributes, validate_output};
use crate::lint_config::{BumpyWeights, RstestHelper};
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
#[case(vec!["rstest:: case".to_owned()], vec!["case".to_owned()])]
#[case(vec![" rstest:: rstest::case ".to_owned()], vec!["case".to_owned()])]
#[case(vec!["rstest::\t rstest:: values ".to_owned()], vec!["values".to_owned()])]
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

/// Generate provider attributes that mix arbitrary text with the cases the
/// canonicalizer must handle: surrounding whitespace, `rstest::` prefixes,
/// empty names, and duplicates.
fn attribute_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => any::<String>(),
        1 => prop::sample::select(vec![
            String::new(),
            " ".to_owned(),
            "\tcase\n".to_owned(),
            "case".to_owned(),
            "rstest::case".to_owned(),
            " rstest::rstest::values ".to_owned(),
            "rstest:: case".to_owned(),
            " rstest:: rstest::case ".to_owned(),
            "rstest::\tvalues".to_owned(),
            "future".to_owned(),
            "future".to_owned(),
        ]),
    ]
}

/// Remove surrounding whitespace and every leading `rstest::` prefix.
///
/// The loop re-trims after each prefix so whitespace revealed by removal is
/// handled in the same pass, matching the documented canonical contract.
fn canonicalize(attribute: &str) -> String {
    let mut canonical = attribute.trim();
    loop {
        match canonical.strip_prefix("rstest::") {
            Some(stripped) => canonical = stripped.trim(),
            None => return canonical.to_owned(),
        }
    }
}

/// Derive the canonical attributes independently of the implementation.
fn expected_provider_attributes(attributes: &[String]) -> Vec<String> {
    let mut expected: Vec<String> = Vec::new();
    for attribute in attributes {
        let canonical = canonicalize(attribute);
        if !canonical.is_empty() && !expected.contains(&canonical) {
            expected.push(canonical);
        }
    }
    if expected.is_empty() {
        RstestHelper::default().provider_param_attributes
    } else {
        expected
    }
}

proptest! {
    /// Keep provider attributes complete, ordered, unique, and idempotent.
    #[test]
    fn normalized_provider_attributes_are_canonical(
        attributes in prop::collection::vec(attribute_strategy(), 0..16)
    ) {
        let normalized = normalize_provider_attributes(attributes.clone());
        prop_assert_eq!(&normalized, &expected_provider_attributes(&attributes));
        prop_assert_eq!(
            normalize_provider_attributes(normalized.clone()),
            normalized
        );
    }

    /// Clamp every bumpy-road tuning value into the lint's valid range.
    #[test]
    fn normalizes_bumpy_road_tuning_to_valid_ranges(
        threshold in any::<f64>(),
        window in any::<usize>(),
        min_bump_lines in any::<usize>(),
        depth in any::<f64>(),
        predicate in any::<f64>(),
        flow in any::<f64>(),
    ) {
        let mut configuration = Configuration::default();
        let bumpy = &mut configuration.bumpy_road_function;
        bumpy.threshold = threshold;
        bumpy.window = window;
        bumpy.min_bump_lines = min_bump_lines;
        bumpy.weights = BumpyWeights { depth, predicate, flow };
        let normalized = normalize(configuration).bumpy_road_function;
        prop_assert!(normalized.threshold.is_finite());
        prop_assert!(normalized.threshold >= 0.0);
        prop_assert!(is_valid_window(normalized.window));
        prop_assert!(normalized.min_bump_lines >= 1);
        for weight in [
            normalized.weights.depth,
            normalized.weights.predicate,
            normalized.weights.flow,
        ] {
            prop_assert!(weight.is_finite());
            prop_assert!(weight >= 0.0);
        }
    }

    /// Preserve bumpy-road tuning that is already inside the valid range.
    #[test]
    fn preserves_valid_bumpy_road_tuning(
        threshold in 0.0f64..1.0e6,
        window in any::<usize>().prop_map(|window| window | 1),
        min_bump_lines in 1usize..1024,
        depth in 0.0f64..1.0e6,
        predicate in 0.0f64..1.0e6,
        flow in 0.0f64..1.0e6,
    ) {
        let mut configuration = Configuration::default();
        let bumpy = &mut configuration.bumpy_road_function;
        bumpy.threshold = threshold;
        bumpy.window = window;
        bumpy.min_bump_lines = min_bump_lines;
        bumpy.weights = BumpyWeights { depth, predicate, flow };
        let normalized = normalize(configuration).bumpy_road_function;
        prop_assert_eq!(normalized.threshold, threshold);
        prop_assert_eq!(normalized.window, window);
        prop_assert_eq!(normalized.min_bump_lines, min_bump_lines);
        prop_assert_eq!(normalized.weights.depth, depth);
        prop_assert_eq!(normalized.weights.predicate, predicate);
        prop_assert_eq!(normalized.weights.flow, flow);
    }
}
