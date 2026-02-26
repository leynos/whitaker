//! Unit tests for brain type diagnostic formatting.

use super::*;
use crate::brain_type_metrics::TypeMetricsBuilder;
use crate::brain_type_metrics::evaluation::BrainTypeDisposition;
use rstest::rstest;

// ---------------------------------------------------------------------------
// Diagnostic — primary message (one brain method)
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

// ---------------------------------------------------------------------------
// Diagnostic — primary message (multiple brain methods)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Diagnostic — primary message (no brain methods)
// ---------------------------------------------------------------------------

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

/// Builds metrics suitable for note and help tests.
fn build_note_metrics(brain_count: usize, lcom4: usize) -> crate::brain_type_metrics::TypeMetrics {
    let cc_threshold = 25;
    let loc_threshold = 80;
    let brain_cc = 30;
    let brain_loc = 100;
    let mut builder = TypeMetricsBuilder::new("Foo", cc_threshold, loc_threshold);

    for i in 0..brain_count {
        builder.add_method(format!("brain_{i}"), brain_cc, brain_loc);
    }
    // Filler to ensure non-zero WMC even without brain methods.
    builder.add_method("filler", 10, 10);
    builder.set_lcom4(lcom4);
    builder.build()
}

#[rstest]
fn note_mentions_wmc() {
    let metrics = build_note_metrics(0, 1);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
    let note = format_note(&diag);
    assert!(note.contains("WMC measures total cognitive complexity"));
}

#[rstest]
fn note_mentions_brain_methods_when_present() {
    let metrics = build_note_metrics(1, 2);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Warn);
    let note = format_note(&diag);
    assert!(note.contains("Brain methods are methods with high complexity"));
}

#[rstest]
fn note_mentions_lcom4_when_low_cohesion() {
    let metrics = build_note_metrics(0, 2);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
    let note = format_note(&diag);
    assert!(note.contains("LCOM4 >= 2"));
}

#[rstest]
fn note_omits_lcom4_when_cohesive() {
    let metrics = build_note_metrics(0, 1);
    let diag = BrainTypeDiagnostic::new(&metrics, BrainTypeDisposition::Pass);
    let note = format_note(&diag);
    assert!(!note.contains("LCOM4 >= 2"));
}

// ---------------------------------------------------------------------------
// Diagnostic — help
// ---------------------------------------------------------------------------

#[rstest]
fn help_suggests_decomposition() {
    let metrics = build_note_metrics(0, 1);
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
