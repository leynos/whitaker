//! Unit tests for brain trait threshold evaluation.

use super::*;
use crate::brain_trait_metrics::TraitMetricsBuilder;
use rstest::rstest;

// ---------------------------------------------------------------------------
// Helper: build TraitMetrics with the desired shape
// ---------------------------------------------------------------------------

/// Builds a `TraitMetrics` with a given number of required and default
/// methods. Each default method receives `cc_per_default` cognitive
/// complexity.
fn build_trait_metrics(
    name: &str,
    required_count: usize,
    default_count: usize,
    cc_per_default: usize,
) -> crate::brain_trait_metrics::TraitMetrics {
    let mut builder = TraitMetricsBuilder::new(name);
    for i in 0..required_count {
        builder.add_required_method(format!("req_{i}"));
    }
    for i in 0..default_count {
        builder.add_default_method(format!("default_{i}"), cc_per_default, false);
    }
    builder.build()
}

/// Builds a `TraitMetrics` with methods plus associated types and consts.
fn build_trait_metrics_with_items(
    name: &str,
    required_count: usize,
    assoc_type_count: usize,
    assoc_const_count: usize,
) -> crate::brain_trait_metrics::TraitMetrics {
    let mut builder = TraitMetricsBuilder::new(name);
    for i in 0..required_count {
        builder.add_required_method(format!("req_{i}"));
    }
    for i in 0..assoc_type_count {
        builder.add_associated_type(format!("Type_{i}"));
    }
    for i in 0..assoc_const_count {
        builder.add_associated_const(format!("CONST_{i}"));
    }
    builder.build()
}

// ---------------------------------------------------------------------------
// Default thresholds
// ---------------------------------------------------------------------------

#[rstest]
#[case("methods_warn", 20)]
#[case("methods_deny", 30)]
#[case("default_cc_warn", 40)]
fn default_threshold_values(#[case] field: &str, #[case] expected: usize) {
    let t = BrainTraitThresholdsBuilder::new().build();
    let actual = match field {
        "methods_warn" => t.methods_warn(),
        "methods_deny" => t.methods_deny(),
        "default_cc_warn" => t.default_cc_warn(),
        _ => panic!("Unknown field: {field}"),
    };
    assert_eq!(actual, expected);
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

#[rstest]
fn builder_overrides_individual_fields() {
    let t = BrainTraitThresholdsBuilder::new().methods_warn(25).build();
    assert_eq!(t.methods_warn(), 25);
    assert_eq!(t.methods_deny(), 30);
    assert_eq!(t.default_cc_warn(), 40);
}

#[rstest]
#[case("methods_warn", 15)]
#[case("methods_deny", 25)]
#[case("default_cc_warn", 50)]
fn builder_chaining_sets_field(#[case] field: &str, #[case] expected: usize) {
    let t = BrainTraitThresholdsBuilder::new()
        .methods_warn(15)
        .methods_deny(25)
        .default_cc_warn(50)
        .build();

    let actual = match field {
        "methods_warn" => t.methods_warn(),
        "methods_deny" => t.methods_deny(),
        "default_cc_warn" => t.default_cc_warn(),
        _ => panic!("Unknown field: {field}"),
    };

    assert_eq!(actual, expected);
}

#[rstest]
fn builder_default_trait_matches_new() {
    let from_new = BrainTraitThresholdsBuilder::new().build();
    let from_default = BrainTraitThresholdsBuilder::default().build();
    assert_eq!(from_new, from_default);
}

// ---------------------------------------------------------------------------
// Evaluation — pass cases
// ---------------------------------------------------------------------------

#[rstest]
#[case("all below thresholds", 5, 5, 2)]
#[case("many methods but low CC", 14, 5, 2)]
#[case("high CC but few methods", 3, 2, 10)]
#[case("at methods_warn but CC below threshold", 15, 5, 1)]
fn evaluate_pass_cases(
    #[case] _label: &str,
    #[case] required: usize,
    #[case] default: usize,
    #[case] cc_per_default: usize,
) {
    let metrics = build_trait_metrics("Foo", required, default, cc_per_default);
    let thresholds = BrainTraitThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_trait(&metrics, &thresholds),
        BrainTraitDisposition::Pass
    );
}

#[rstest]
fn pass_when_at_methods_warn_but_cc_one_below() {
    // 20 methods total (15+5), CC sum = 5*7 = 35 < 40.
    let metrics = build_trait_metrics("Foo", 15, 5, 7);
    let thresholds = BrainTraitThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_trait(&metrics, &thresholds),
        BrainTraitDisposition::Pass
    );
}

// ---------------------------------------------------------------------------
// Evaluation — warn cases
// ---------------------------------------------------------------------------

#[rstest]
#[case("exact warn thresholds", 12, 8, 5)]
#[case("above warn below deny", 15, 10, 6)]
#[case("just below methods_deny", 19, 10, 4)]
fn evaluate_warn_cases(
    #[case] _label: &str,
    #[case] required: usize,
    #[case] default: usize,
    #[case] cc_per_default: usize,
) {
    let metrics = build_trait_metrics("Bar", required, default, cc_per_default);
    let thresholds = BrainTraitThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_trait(&metrics, &thresholds),
        BrainTraitDisposition::Warn
    );
}

// ---------------------------------------------------------------------------
// Evaluation — deny cases
// ---------------------------------------------------------------------------

#[rstest]
#[case("method count at deny threshold", 20, 10, 0)]
#[case("method count above deny", 25, 10, 0)]
#[case("deny supersedes warn", 20, 10, 5)]
fn evaluate_deny_cases(
    #[case] _label: &str,
    #[case] required: usize,
    #[case] default: usize,
    #[case] cc_per_default: usize,
) {
    let metrics = build_trait_metrics("Baz", required, default, cc_per_default);
    let thresholds = BrainTraitThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_trait(&metrics, &thresholds),
        BrainTraitDisposition::Deny
    );
}

// ---------------------------------------------------------------------------
// Evaluation — custom thresholds
// ---------------------------------------------------------------------------

#[rstest]
#[case(
    "custom methods_warn",
    (10, 5, 8),
    BrainTraitThresholdsBuilder::new().methods_warn(15).build(),
    BrainTraitDisposition::Warn
)]
#[case(
    "custom methods_deny",
    (15, 5, 0),
    BrainTraitThresholdsBuilder::new().methods_deny(20).build(),
    BrainTraitDisposition::Deny
)]
#[case(
    "custom default_cc_warn",
    (15, 5, 6),
    BrainTraitThresholdsBuilder::new().default_cc_warn(30).build(),
    BrainTraitDisposition::Warn
)]
fn custom_threshold_overrides(
    #[case] _label: &str,
    #[case] shape: (usize, usize, usize),
    #[case] thresholds: BrainTraitThresholds,
    #[case] expected: BrainTraitDisposition,
) {
    let (required, default, cc_per_default) = shape;
    let metrics = build_trait_metrics("Custom", required, default, cc_per_default);
    assert_eq!(evaluate_brain_trait(&metrics, &thresholds), expected);
}

// ---------------------------------------------------------------------------
// Evaluation — associated items do not count as methods
// ---------------------------------------------------------------------------

#[rstest]
fn associated_items_excluded_from_method_count() {
    // 19 required methods + 5 types + 5 consts = 29 total items but
    // only 19 methods. methods_warn is 20, so this should pass.
    let metrics = build_trait_metrics_with_items("ItemHeavy", 19, 5, 5);
    let thresholds = BrainTraitThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_trait(&metrics, &thresholds),
        BrainTraitDisposition::Pass,
        "associated types and consts must not count toward method thresholds"
    );
}
