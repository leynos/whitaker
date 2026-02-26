//! Unit tests for brain type threshold evaluation.

use super::*;
use crate::brain_type_metrics::TypeMetricsBuilder;
use rstest::rstest;

// ---------------------------------------------------------------------------
// Helper: build TypeMetrics with the desired shape
// ---------------------------------------------------------------------------

/// Builds a `TypeMetrics` with a given WMC, brain method count, and LCOM4.
///
/// Brain methods are synthesized by adding methods with CC=30, LOC=100
/// (above default thresholds of CC=25, LOC=80). Non-brain filler methods
/// contribute CC to reach the desired WMC total.
fn build_metrics(name: &str, target_wmc: usize, brain_count: usize, lcom4: usize) -> TypeMetrics {
    let cc_threshold = 25;
    let loc_threshold = 80;
    let brain_cc = 30;
    let brain_loc = 100;
    let mut builder = TypeMetricsBuilder::new(name, cc_threshold, loc_threshold);

    let brain_total_cc = brain_count * brain_cc;
    for i in 0..brain_count {
        builder.add_method(format!("brain_{i}"), brain_cc, brain_loc);
    }

    // Fill remaining WMC with a single non-brain filler method if needed.
    if target_wmc > brain_total_cc {
        let filler_cc = target_wmc - brain_total_cc;
        builder.add_method("filler", filler_cc, 10);
    }

    builder.set_lcom4(lcom4);
    builder.build()
}

// ---------------------------------------------------------------------------
// Default thresholds
// ---------------------------------------------------------------------------

#[rstest]
#[case("wmc_warn", 60)]
#[case("wmc_deny", 100)]
#[case("lcom4_warn", 2)]
#[case("lcom4_deny", 3)]
#[case("brain_method_deny_count", 2)]
fn default_threshold_values(#[case] field: &str, #[case] expected: usize) {
    let t = BrainTypeThresholdsBuilder::new().build();
    let actual = match field {
        "wmc_warn" => t.wmc_warn(),
        "wmc_deny" => t.wmc_deny(),
        "lcom4_warn" => t.lcom4_warn(),
        "lcom4_deny" => t.lcom4_deny(),
        "brain_method_deny_count" => t.brain_method_deny_count(),
        _ => panic!("Unknown field: {field}"),
    };
    assert_eq!(actual, expected);
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

#[rstest]
fn builder_overrides_individual_fields() {
    let t = BrainTypeThresholdsBuilder::new().wmc_warn(50).build();
    assert_eq!(t.wmc_warn(), 50);
    assert_eq!(t.wmc_deny(), 100);
    assert_eq!(t.lcom4_warn(), 2);
}

#[rstest]
#[case("wmc_warn", 40)]
#[case("wmc_deny", 80)]
#[case("lcom4_warn", 3)]
#[case("lcom4_deny", 5)]
#[case("brain_method_deny_count", 3)]
fn builder_chaining_sets_field(#[case] field: &str, #[case] expected: usize) {
    let t = BrainTypeThresholdsBuilder::new()
        .wmc_warn(40)
        .wmc_deny(80)
        .lcom4_warn(3)
        .lcom4_deny(5)
        .brain_method_deny_count(3)
        .build();

    let actual = match field {
        "wmc_warn" => t.wmc_warn(),
        "wmc_deny" => t.wmc_deny(),
        "lcom4_warn" => t.lcom4_warn(),
        "lcom4_deny" => t.lcom4_deny(),
        "brain_method_deny_count" => t.brain_method_deny_count(),
        _ => panic!("Unknown field: {field}"),
    };

    assert_eq!(actual, expected);
}

#[rstest]
fn builder_default_trait_matches_new() {
    let from_new = BrainTypeThresholdsBuilder::new().build();
    let from_default = BrainTypeThresholdsBuilder::default().build();
    assert_eq!(from_new, from_default);
}

// ---------------------------------------------------------------------------
// Evaluation — pass cases
// ---------------------------------------------------------------------------

#[rstest]
#[case("all below thresholds", 30, 0, 1)]
#[case("high WMC but no brain method and cohesive", 80, 0, 1)]
#[case("brain method present but cohesive (LCOM4=1)", 80, 1, 1)]
#[case("brain method and low cohesion but low WMC", 30, 1, 2)]
fn evaluate_pass_cases(
    #[case] _label: &str,
    #[case] wmc: usize,
    #[case] brain_count: usize,
    #[case] lcom4: usize,
) {
    let metrics = build_metrics("Foo", wmc, brain_count, lcom4);
    let thresholds = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_type(&metrics, &thresholds),
        BrainTypeDisposition::Pass
    );
}

// ---------------------------------------------------------------------------
// Evaluation — warn cases
// ---------------------------------------------------------------------------

#[rstest]
#[case("exact warn thresholds", 60, 1, 2)]
#[case("above warn below deny", 80, 1, 2)]
#[case("just below WMC deny", 99, 1, 2)]
fn evaluate_warn_cases(
    #[case] _label: &str,
    #[case] wmc: usize,
    #[case] brain_count: usize,
    #[case] lcom4: usize,
) {
    let metrics = build_metrics("Bar", wmc, brain_count, lcom4);
    let thresholds = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_type(&metrics, &thresholds),
        BrainTypeDisposition::Warn
    );
}

// ---------------------------------------------------------------------------
// Evaluation — deny cases
// ---------------------------------------------------------------------------

#[rstest]
#[case("WMC at deny threshold", 100, 0, 1)]
#[case("multiple brain methods trigger deny", 60, 2, 1)]
#[case("LCOM4 at deny threshold", 30, 0, 3)]
#[case("deny supersedes warn", 100, 1, 3)]
#[case("all deny triggers active", 100, 2, 3)]
fn evaluate_deny_cases(
    #[case] _label: &str,
    #[case] wmc: usize,
    #[case] brain_count: usize,
    #[case] lcom4: usize,
) {
    let metrics = build_metrics("Baz", wmc, brain_count, lcom4);
    let thresholds = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_type(&metrics, &thresholds),
        BrainTypeDisposition::Deny
    );
}

// ---------------------------------------------------------------------------
// Evaluation — custom thresholds
// ---------------------------------------------------------------------------

#[rstest]
fn custom_wmc_warn_threshold() {
    let metrics = build_metrics("Custom", 40, 1, 2);
    let thresholds = BrainTypeThresholdsBuilder::new().wmc_warn(40).build();
    assert_eq!(
        evaluate_brain_type(&metrics, &thresholds),
        BrainTypeDisposition::Warn
    );
}

#[rstest]
fn custom_deny_thresholds() {
    let metrics = build_metrics("Custom", 50, 0, 4);
    let thresholds = BrainTypeThresholdsBuilder::new().lcom4_deny(4).build();
    assert_eq!(
        evaluate_brain_type(&metrics, &thresholds),
        BrainTypeDisposition::Deny
    );
}

// ---------------------------------------------------------------------------
// Evaluation — brain_method_deny_count boundary
// ---------------------------------------------------------------------------

#[rstest]
fn one_brain_method_does_not_trigger_deny() {
    // 1 brain method is below the default deny count of 2.
    let metrics = build_metrics("BM", 30, 1, 1);
    let thresholds = BrainTypeThresholdsBuilder::new().build();
    assert_ne!(
        evaluate_brain_type(&metrics, &thresholds),
        BrainTypeDisposition::Deny
    );
}

#[rstest]
fn exact_brain_method_deny_count_triggers_deny() {
    // Exactly 2 brain methods (the default deny count) triggers deny.
    let metrics = build_metrics("BM", 60, 2, 1);
    let thresholds = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(
        evaluate_brain_type(&metrics, &thresholds),
        BrainTypeDisposition::Deny
    );
}

#[rstest]
#[case("below custom threshold", 2, false)]
#[case("at custom threshold boundary", 3, true)]
fn custom_brain_method_deny_count_boundary(
    #[case] _label: &str,
    #[case] brain_count: usize,
    #[case] should_deny: bool,
) {
    // Override deny count to 3, test boundary behaviour.
    // Use WMC and LCOM4 values that won't trigger their own deny conditions.
    let metrics = build_metrics("BM", 90, brain_count, 1);
    let thresholds = BrainTypeThresholdsBuilder::new()
        .brain_method_deny_count(3)
        .build();

    let disposition = evaluate_brain_type(&metrics, &thresholds);
    if should_deny {
        assert_eq!(disposition, BrainTypeDisposition::Deny);
    } else {
        assert_ne!(disposition, BrainTypeDisposition::Deny);
    }
}
