//! Unit tests for brain type threshold evaluation and diagnostic formatting.

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
fn default_wmc_warn_threshold() {
    let t = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(t.wmc_warn(), 60);
}

#[rstest]
fn default_wmc_deny_threshold() {
    let t = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(t.wmc_deny(), 100);
}

#[rstest]
fn default_lcom4_warn_threshold() {
    let t = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(t.lcom4_warn(), 2);
}

#[rstest]
fn default_lcom4_deny_threshold() {
    let t = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(t.lcom4_deny(), 3);
}

#[rstest]
fn default_brain_method_deny_count_threshold() {
    let t = BrainTypeThresholdsBuilder::new().build();
    assert_eq!(t.brain_method_deny_count(), 2);
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
fn builder_chaining_sets_wmc_warn() {
    let t = BrainTypeThresholdsBuilder::new()
        .wmc_warn(40)
        .wmc_deny(80)
        .lcom4_warn(3)
        .lcom4_deny(5)
        .brain_method_deny_count(3)
        .build();
    assert_eq!(t.wmc_warn(), 40);
}

#[rstest]
fn builder_chaining_sets_wmc_deny() {
    let t = BrainTypeThresholdsBuilder::new()
        .wmc_warn(40)
        .wmc_deny(80)
        .lcom4_warn(3)
        .lcom4_deny(5)
        .brain_method_deny_count(3)
        .build();
    assert_eq!(t.wmc_deny(), 80);
}

#[rstest]
fn builder_chaining_sets_lcom4_warn() {
    let t = BrainTypeThresholdsBuilder::new()
        .wmc_warn(40)
        .wmc_deny(80)
        .lcom4_warn(3)
        .lcom4_deny(5)
        .brain_method_deny_count(3)
        .build();
    assert_eq!(t.lcom4_warn(), 3);
}

#[rstest]
fn builder_chaining_sets_lcom4_deny() {
    let t = BrainTypeThresholdsBuilder::new()
        .wmc_warn(40)
        .wmc_deny(80)
        .lcom4_warn(3)
        .lcom4_deny(5)
        .brain_method_deny_count(3)
        .build();
    assert_eq!(t.lcom4_deny(), 5);
}

#[rstest]
fn builder_chaining_sets_brain_method_deny_count() {
    let t = BrainTypeThresholdsBuilder::new()
        .wmc_warn(40)
        .wmc_deny(80)
        .lcom4_warn(3)
        .lcom4_deny(5)
        .brain_method_deny_count(3)
        .build();
    assert_eq!(t.brain_method_deny_count(), 3);
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
// Diagnostic — primary message
// ---------------------------------------------------------------------------

/// Builds a diagnostic message for a type with one brain method (`parse_all`,
/// CC=31, LOC=140) and a non-brain helper (CC=5, LOC=20), LCOM4=3.
fn one_brain_method_message() -> String {
    let mut builder = TypeMetricsBuilder::new("Foo", 25, 80);
    builder.add_method("parse_all", 31, 140);
    builder.add_method("helper", 5, 20);
    builder.set_lcom4(3);
    let metrics = builder.build();
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Warn);
    format_primary_message(&diag)
}

#[rstest]
fn one_brain_method_message_contains_type_name() {
    let msg = one_brain_method_message();
    assert!(msg.contains("`Foo`"), "should contain type name");
}

#[rstest]
fn one_brain_method_message_contains_wmc() {
    let msg = one_brain_method_message();
    assert!(msg.contains("WMC=36"), "should contain WMC value");
}

#[rstest]
fn one_brain_method_message_contains_lcom4() {
    let msg = one_brain_method_message();
    assert!(msg.contains("LCOM4=3"), "should contain LCOM4 value");
}

#[rstest]
fn one_brain_method_message_contains_method_name() {
    let msg = one_brain_method_message();
    assert!(
        msg.contains("`parse_all`"),
        "should contain brain method name"
    );
}

#[rstest]
fn one_brain_method_message_contains_cc() {
    let msg = one_brain_method_message();
    assert!(msg.contains("CC=31"), "should contain brain method CC");
}

#[rstest]
fn one_brain_method_message_contains_loc() {
    let msg = one_brain_method_message();
    assert!(msg.contains("LOC=140"), "should contain brain method LOC");
}

#[rstest]
fn one_brain_method_message_uses_singular_form() {
    let msg = one_brain_method_message();
    assert!(msg.contains("a brain method"), "should use singular form");
}

/// Builds a diagnostic message for a type with two brain methods (`parse`
/// CC=30, LOC=100 and `render` CC=40, LOC=200), LCOM4=2.
fn multiple_brain_methods_message() -> String {
    let mut builder = TypeMetricsBuilder::new("Bar", 25, 80);
    builder.add_method("parse", 30, 100);
    builder.add_method("render", 40, 200);
    builder.set_lcom4(2);
    let metrics = builder.build();
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Deny);
    format_primary_message(&diag)
}

#[rstest]
fn multiple_brain_methods_message_uses_plural_form() {
    let msg = multiple_brain_methods_message();
    assert!(msg.contains("2 brain methods"), "should use plural form");
}

#[rstest]
fn multiple_brain_methods_message_lists_first_method() {
    let msg = multiple_brain_methods_message();
    assert!(msg.contains("`parse`"), "should list first brain method");
}

#[rstest]
fn multiple_brain_methods_message_lists_second_method() {
    let msg = multiple_brain_methods_message();
    assert!(msg.contains("`render`"), "should list second brain method");
}

#[rstest]
fn multiple_brain_methods_message_contains_first_cc() {
    let msg = multiple_brain_methods_message();
    assert!(msg.contains("CC=30"), "should contain first method CC");
}

#[rstest]
fn multiple_brain_methods_message_contains_second_loc() {
    let msg = multiple_brain_methods_message();
    assert!(msg.contains("LOC=200"), "should contain second method LOC");
}

#[rstest]
fn primary_message_with_no_brain_methods() {
    let mut builder = TypeMetricsBuilder::new("Simple", 25, 80);
    builder.add_method("a", 10, 20);
    builder.set_lcom4(1);
    let metrics = builder.build();
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
    let msg = format_primary_message(&diag);

    assert!(msg.contains("WMC=10"), "should contain WMC");
    assert!(msg.contains("LCOM4=1"), "should contain LCOM4");
    assert!(
        !msg.contains("brain method"),
        "should not mention brain methods"
    );
}

// ---------------------------------------------------------------------------
// Diagnostic — note
// ---------------------------------------------------------------------------

#[rstest]
fn note_mentions_wmc() {
    let metrics = build_metrics("Foo", 30, 0, 1);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
    let note = format_note(&diag);
    assert!(note.contains("WMC measures total cognitive complexity"));
}

#[rstest]
fn note_mentions_brain_methods_when_present() {
    let metrics = build_metrics("Foo", 60, 1, 2);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Warn);
    let note = format_note(&diag);
    assert!(note.contains("Brain methods are methods with high complexity"));
}

#[rstest]
fn note_mentions_lcom4_when_low_cohesion() {
    let metrics = build_metrics("Foo", 30, 0, 2);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
    let note = format_note(&diag);
    assert!(note.contains("LCOM4 >= 2"));
}

#[rstest]
fn note_omits_lcom4_when_cohesive() {
    let metrics = build_metrics("Foo", 30, 0, 1);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
    let note = format_note(&diag);
    assert!(!note.contains("LCOM4 >= 2"));
}

// ---------------------------------------------------------------------------
// Diagnostic — help
// ---------------------------------------------------------------------------

#[rstest]
fn help_suggests_decomposition() {
    let metrics = build_metrics("Foo", 30, 0, 1);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
    let help = format_help(&diag);
    assert!(help.contains("extracting related methods"));
}

// ---------------------------------------------------------------------------
// Diagnostic — accessors
// ---------------------------------------------------------------------------

/// Builds a diagnostic for accessor tests: type "Qux" with one brain method
/// "big" (CC=30, LOC=100), LCOM4=2, foreign reach=7.
fn accessor_diagnostic() -> BrainTypeDiagnostic {
    let mut builder = TypeMetricsBuilder::new("Qux", 25, 80);
    builder.add_method("big", 30, 100);
    builder.set_lcom4(2);
    builder.set_foreign_reach(7);
    let metrics = builder.build();
    BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Warn)
}

#[rstest]
fn diagnostic_type_name_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.type_name(), "Qux");
}

#[rstest]
fn diagnostic_disposition_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.disposition(), BrainTypeDisposition::Warn);
}

#[rstest]
fn diagnostic_wmc_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.wmc(), 30);
}

#[rstest]
fn diagnostic_lcom4_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.lcom4(), 2);
}

#[rstest]
fn diagnostic_foreign_reach_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.foreign_reach(), 7);
}

#[rstest]
fn diagnostic_brain_methods_count_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.brain_methods().len(), 1);
}

#[rstest]
fn diagnostic_brain_methods_name_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.brain_methods()[0].name(), "big");
}
