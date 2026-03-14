//! Unit tests for brain trait diagnostic formatting.

use super::*;
use crate::brain_trait_metrics::TraitMetricsBuilder;
use crate::brain_trait_metrics::evaluation::BrainTraitDisposition;
use crate::decomposition_advice::{
    DecompositionContext, MethodProfileBuilder, SubjectKind, suggest_decomposition,
};
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
#[case("Foo", (5, 5, 4),  BrainTraitDisposition::Warn, "interface size")]
#[case("Foo", (5, 5, 4),  BrainTraitDisposition::Warn, "Default method CC sum")]
#[case("Foo", (10, 0, 0), BrainTraitDisposition::Pass, "Implementor burden")]
fn note_contains_expected_fragment(
    #[case] name: &str,
    #[case] shape: (usize, usize, usize),
    #[case] disposition: BrainTraitDisposition,
    #[case] fragment: &str,
) {
    let (required, default, cc_per_default) = shape;
    let diag = build_diagnostic(DiagnosticInput {
        name,
        required,
        default,
        cc_per_default,
        disposition,
    });
    assert!(
        format_note(&diag).contains(fragment),
        "missing fragment: {fragment}"
    );
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
    assert!(
        !format_note(&diag).contains("Default method CC"),
        "should not mention CC when no default methods",
    );
}

// ---------------------------------------------------------------------------
// Diagnostic - decomposition note
// ---------------------------------------------------------------------------

#[rstest]
fn decomposition_note_delegates_to_shared_renderer_for_traits() {
    let diagnostic = build_diagnostic(DiagnosticInput {
        name: "Transport",
        required: 2,
        default: 2,
        cc_per_default: 5,
        disposition: BrainTraitDisposition::Warn,
    });
    let context = DecompositionContext::new(diagnostic.trait_name(), SubjectKind::Trait);

    let mut encode_request = MethodProfileBuilder::new("encode_request");
    encode_request.record_external_domain("serde::json");

    let mut decode_request = MethodProfileBuilder::new("decode_request");
    decode_request.record_external_domain("serde::json");

    let mut read_frame = MethodProfileBuilder::new("read_frame");
    read_frame.record_external_domain("std::io");

    let mut write_frame = MethodProfileBuilder::new("write_frame");
    write_frame.record_external_domain("std::io");

    let suggestions = suggest_decomposition(
        &context,
        &[
            encode_request.build(),
            decode_request.build(),
            read_frame.build(),
            write_frame.build(),
        ],
    );

    assert_eq!(
        format_decomposition_note(&diagnostic, &suggestions),
        Some(
            concat!(
                "Potential decomposition for `Transport`:\n",
                "- [serde::json] sub-trait for `decode_request`, `encode_request`\n",
                "- [std::io] sub-trait for `read_frame`, `write_frame`",
            )
            .to_owned()
        )
    );
}

// ---------------------------------------------------------------------------
// Diagnostic — help
// ---------------------------------------------------------------------------

#[rstest]
#[case("Big",     (15, 10, 4), BrainTraitDisposition::Warn, "splitting the trait into focused sub-traits")]
#[case("Complex", (5, 10, 5),  BrainTraitDisposition::Warn, "extracting complex default method bodies")]
#[case("Heavy",   (15, 0, 0),  BrainTraitDisposition::Pass, "default implementations to reduce implementor burden")]
#[case("Empty",   (0, 0, 0),   BrainTraitDisposition::Pass, "splitting the trait into smaller")]
fn help_suggestions(
    #[case] name: &str,
    #[case] shape: (usize, usize, usize),
    #[case] disposition: BrainTraitDisposition,
    #[case] fragment: &str,
) {
    let (required, default, cc_per_default) = shape;
    let diag = build_diagnostic(DiagnosticInput {
        name,
        required,
        default,
        cc_per_default,
        disposition,
    });
    assert!(
        format_help(&diag).contains(fragment),
        "missing fragment: {fragment}"
    );
}

// ---------------------------------------------------------------------------
// Diagnostic — total_item_count with associated types and consts
// ---------------------------------------------------------------------------

#[rstest]
fn total_item_count_includes_associated_items() {
    // 3 required methods + 2 associated types + 1 associated const = 6 items,
    // but only 3 methods.
    let mut builder = TraitMetricsBuilder::new("Mixed");
    for i in 0..3 {
        builder.add_required_method(format!("req_{i}"));
    }
    builder.add_associated_type("Output");
    builder.add_associated_type("Error");
    builder.add_associated_const("VERSION");
    let metrics = builder.build();
    let diag = BrainTraitDiagnostic::new(&metrics, BrainTraitDisposition::Pass);
    assert_eq!(
        diag.total_item_count(),
        6,
        "should count methods + types + consts"
    );
    assert_eq!(diag.total_method_count(), 3, "should count only methods");
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
#[case("required_method_count", 10)]
#[case("default_method_count", 5)]
#[case("total_method_count", 15)]
#[case("default_method_cc_sum", 20)]
#[case("total_item_count", 15)]
#[case("implementor_burden", 10)]
fn diagnostic_usize_accessors(#[case] field: &str, #[case] expected: usize) {
    let diag = accessor_diagnostic();
    let actual = match field {
        "required_method_count" => diag.required_method_count(),
        "default_method_count" => diag.default_method_count(),
        "total_method_count" => diag.total_method_count(),
        "default_method_cc_sum" => diag.default_method_cc_sum(),
        "total_item_count" => diag.total_item_count(),
        "implementor_burden" => diag.implementor_burden(),
        _ => panic!("Unknown field: {field}"),
    };
    assert_eq!(actual, expected);
}
