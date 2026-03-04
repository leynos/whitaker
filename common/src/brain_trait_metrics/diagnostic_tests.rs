//! Unit tests for brain trait diagnostic formatting.

use super::*;
use crate::brain_trait_metrics::TraitMetricsBuilder;
use crate::brain_trait_metrics::evaluation::BrainTraitDisposition;
use rstest::rstest;

// ---------------------------------------------------------------------------
// Helper: build diagnostics for formatting tests
// ---------------------------------------------------------------------------

struct DiagnosticInput<'a> {
    name: &'a str,
    required: usize,
    default: usize,
    cc_per_default: usize,
    disposition: BrainTraitDisposition,
}

/// Builds a diagnostic for a trait with the given method breakdown.
fn build_diagnostic(input: DiagnosticInput<'_>) -> BrainTraitDiagnostic {
    let mut builder = TraitMetricsBuilder::new(input.name);
    for i in 0..input.required {
        builder.add_required_method(format!("req_{i}"));
    }
    for i in 0..input.default {
        builder.add_default_method(format!("default_{i}"), input.cc_per_default, false);
    }
    let metrics = builder.build();
    BrainTraitDiagnostic::new(&metrics, input.disposition)
}

// ---------------------------------------------------------------------------
// Diagnostic — primary message (with default methods)
// ---------------------------------------------------------------------------

/// Builds a primary message for a trait with 15 required + 10 default
/// methods, each default having CC=5 (CC sum=50).
fn primary_message_with_defaults() -> String {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Parser",
        required: 15,
        default: 10,
        cc_per_default: 5,
        disposition: BrainTraitDisposition::Warn,
    });
    format_primary_message(&diag)
}

#[rstest]
#[case("`Parser`", "trait name")]
#[case("25 methods", "total method count")]
#[case("15 required", "required count")]
#[case("10 default", "default count")]
#[case("CC=50", "CC sum")]
fn primary_message_with_defaults_contains(#[case] fragment: &str, #[case] _description: &str) {
    let msg = primary_message_with_defaults();
    assert!(msg.contains(fragment), "missing fragment: {fragment}");
}

// ---------------------------------------------------------------------------
// Diagnostic — primary message (without default methods)
// ---------------------------------------------------------------------------

#[rstest]
fn primary_message_omits_cc_when_zero() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Simple",
        required: 10,
        default: 0,
        cc_per_default: 0,
        disposition: BrainTraitDisposition::Pass,
    });
    let msg = format_primary_message(&diag);
    assert!(!msg.contains("CC="), "should not mention CC when zero");
    assert!(
        msg.contains("10 methods"),
        "should contain total method count"
    );
}

// ---------------------------------------------------------------------------
// Diagnostic — primary message (only default methods)
// ---------------------------------------------------------------------------

#[rstest]
fn primary_message_with_only_default_methods() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "AllDefault",
        required: 0,
        default: 5,
        cc_per_default: 8,
        disposition: BrainTraitDisposition::Pass,
    });
    let msg = format_primary_message(&diag);
    assert!(msg.contains("0 required"), "should show zero required");
    assert!(msg.contains("5 default"), "should show default count");
    assert!(msg.contains("CC=40"), "should show CC sum");
}

// ---------------------------------------------------------------------------
// Diagnostic — note
// ---------------------------------------------------------------------------

#[rstest]
fn note_mentions_interface_size() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Foo",
        required: 5,
        default: 5,
        cc_per_default: 4,
        disposition: BrainTraitDisposition::Warn,
    });
    let note = format_note(&diag);
    assert!(note.contains("interface size"));
}

#[rstest]
fn note_mentions_cc_when_nonzero() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Foo",
        required: 5,
        default: 5,
        cc_per_default: 4,
        disposition: BrainTraitDisposition::Warn,
    });
    let note = format_note(&diag);
    assert!(note.contains("Default method CC sum"));
}

#[rstest]
fn note_omits_cc_when_no_default_methods() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Foo",
        required: 10,
        default: 0,
        cc_per_default: 0,
        disposition: BrainTraitDisposition::Pass,
    });
    let note = format_note(&diag);
    assert!(
        !note.contains("Default method CC"),
        "should not mention CC when no default methods"
    );
}

#[rstest]
fn note_mentions_implementor_burden() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Foo",
        required: 10,
        default: 0,
        cc_per_default: 0,
        disposition: BrainTraitDisposition::Pass,
    });
    let note = format_note(&diag);
    assert!(note.contains("Implementor burden"));
}

// ---------------------------------------------------------------------------
// Diagnostic — help
// ---------------------------------------------------------------------------

#[rstest]
fn help_suggests_splitting_when_methods_present() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Big",
        required: 15,
        default: 10,
        cc_per_default: 4,
        disposition: BrainTraitDisposition::Warn,
    });
    let help = format_help(&diag);
    assert!(help.contains("splitting the trait into focused sub-traits"));
}

#[rstest]
fn help_suggests_extracting_defaults_when_cc_nonzero() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Complex",
        required: 5,
        default: 10,
        cc_per_default: 5,
        disposition: BrainTraitDisposition::Warn,
    });
    let help = format_help(&diag);
    assert!(help.contains("extracting complex default method bodies"));
}

#[rstest]
fn help_suggests_reducing_burden_when_required_present() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Heavy",
        required: 15,
        default: 0,
        cc_per_default: 0,
        disposition: BrainTraitDisposition::Pass,
    });
    let help = format_help(&diag);
    assert!(help.contains("default implementations to reduce implementor burden"));
}

#[rstest]
fn help_provides_fallback_when_empty_trait() {
    let diag = build_diagnostic(DiagnosticInput {
        name: "Empty",
        required: 0,
        default: 0,
        cc_per_default: 0,
        disposition: BrainTraitDisposition::Pass,
    });
    let help = format_help(&diag);
    assert!(help.contains("splitting the trait into smaller"));
}

// ---------------------------------------------------------------------------
// Diagnostic — accessors
// ---------------------------------------------------------------------------

/// Builds a diagnostic for accessor tests: trait "Qux" with 10
/// required, 5 default (CC=4 each), CC sum=20.
fn accessor_diagnostic() -> BrainTraitDiagnostic {
    build_diagnostic(DiagnosticInput {
        name: "Qux",
        required: 10,
        default: 5,
        cc_per_default: 4,
        disposition: BrainTraitDisposition::Warn,
    })
}

#[rstest]
fn diagnostic_trait_name_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.trait_name(), "Qux");
}

#[rstest]
fn diagnostic_disposition_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.disposition(), BrainTraitDisposition::Warn);
}

#[rstest]
fn diagnostic_required_method_count_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.required_method_count(), 10);
}

#[rstest]
fn diagnostic_default_method_count_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.default_method_count(), 5);
}

#[rstest]
fn diagnostic_total_method_count_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.total_method_count(), 15);
}

#[rstest]
fn diagnostic_cc_sum_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.default_method_cc_sum(), 20);
}

#[rstest]
fn diagnostic_total_item_count_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.total_item_count(), 15);
}

#[rstest]
fn diagnostic_implementor_burden_accessor() {
    let diag = accessor_diagnostic();
    assert_eq!(diag.implementor_burden(), 10);
}
