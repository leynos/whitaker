//! Compile-pass checks for const public common APIs in an external crate.

use whitaker_common::attributes::Attribute;
use whitaker_common::brain_trait_metrics::{TraitItemMetrics, TraitMetrics, trait_item_count};
use whitaker_common::brain_type_metrics::MethodMetrics;
use whitaker_common::complexity_signal::{LineSegment, SegmentError};
use whitaker_common::span::{
    SourceLocation, SourceSpan, span_line_count, span_to_lines,
};
use whitaker_common::{
    AttributeKind, BrainTraitDiagnostic, BrainTraitThresholdsBuilder, BrainTypeDiagnostic,
    BrainTypeThresholdsBuilder, CognitiveComplexityBuilder, LocaleSelection, Localizer, TypeMetrics,
};

const START: SourceLocation = SourceLocation::new(2, 3);
const END: SourceLocation = SourceLocation::new(4, 5);
const SPAN: Result<SourceSpan, whitaker_common::span::SpanError> =
    SourceSpan::new(START, END);
const SPAN_START_LINE: usize = match SPAN {
    Ok(span) => span.start().line(),
    Err(_) => 0,
};
const SEGMENT: Result<LineSegment, SegmentError> = LineSegment::new(3, 5, 1.25);
const SEGMENT_END: usize = match SEGMENT {
    Ok(segment) => segment.end_line(),
    Err(_) => 0,
};
const TRAIT_WARN: usize = BrainTraitThresholdsBuilder::new()
    .methods_warn(25)
    .methods_deny(35)
    .default_cc_warn(45)
    .build()
    .methods_warn();
const TRAIT_DENY: usize = BrainTraitThresholdsBuilder::new()
    .methods_warn(25)
    .methods_deny(35)
    .default_cc_warn(45)
    .build()
    .methods_deny();
const TRAIT_DEFAULT_CC_WARN: usize = BrainTraitThresholdsBuilder::new()
    .methods_warn(25)
    .methods_deny(35)
    .default_cc_warn(45)
    .build()
    .default_cc_warn();
const TYPE_DENY: usize = BrainTypeThresholdsBuilder::new()
    .wmc_warn(40)
    .wmc_deny(90)
    .lcom4_warn(2)
    .lcom4_deny(4)
    .brain_method_deny_count(3)
    .build()
    .wmc_deny();
const COMPLEXITY: CognitiveComplexityBuilder = CognitiveComplexityBuilder::new();

/// Provide an empty item slice that a const context can inspect.
const NO_TRAIT_ITEMS: &[TraitItemMetrics] = &[];
const TRAIT_ITEM_TOTAL: usize = trait_item_count(NO_TRAIT_ITEMS);

/// Read the builder score in a const context.
const fn complexity_score(builder: &CognitiveComplexityBuilder) -> usize {
    builder.score()
}

/// Read the trait metrics accessors in a const context.
const fn trait_metrics_shape(metrics: &TraitMetrics) -> usize {
    metrics.total_item_count()
        + metrics.required_method_count()
        + metrics.default_method_count()
        + metrics.default_method_cc_sum()
        + metrics.implementor_burden()
}

/// Read the method metrics accessors in a const context.
const fn method_metrics_shape(metrics: &MethodMetrics) -> usize {
    metrics.cognitive_complexity() + metrics.lines_of_code()
}

/// Evaluate the method metrics predicate in a const context.
const fn method_is_brain(metrics: &MethodMetrics, cc: usize, loc: usize) -> bool {
    metrics.is_brain_method(cc, loc)
}

/// Read the type metrics accessors in a const context.
const fn type_metrics_shape(metrics: &TypeMetrics) -> usize {
    metrics.wmc()
        + metrics.brain_method_count()
        + metrics.lcom4()
        + metrics.foreign_reach()
        + metrics.method_count()
}

/// Read the trait diagnostic accessors in a const context.
const fn trait_diagnostic_shape(diagnostic: &BrainTraitDiagnostic) -> usize {
    let _disposition = diagnostic.disposition();
    diagnostic.required_method_count()
        + diagnostic.default_method_count()
        + diagnostic.total_method_count()
        + diagnostic.default_method_cc_sum()
        + diagnostic.total_item_count()
        + diagnostic.implementor_burden()
}

/// Read the type diagnostic accessors in a const context.
const fn type_diagnostic_shape(diagnostic: &BrainTypeDiagnostic) -> usize {
    let _disposition = diagnostic.disposition();
    diagnostic.wmc() + diagnostic.lcom4() + diagnostic.foreign_reach()
}

/// Read the attribute accessors in a const context.
const fn attribute_shape(attribute: &Attribute) -> bool {
    let _path = attribute.path();
    attribute.kind().is_inner() || attribute.is_inner() || attribute.is_outer()
}

/// Read the span helpers in a const context.
const fn span_shape(span: SourceSpan) -> usize {
    span_line_count(span) + *span_to_lines(span).start()
}

/// Check locale accessors in a const context without constructing a localizer.
const fn locale_fallback(selection: &LocaleSelection) -> bool {
    selection.localizer().used_fallback() || selection.used_fallback()
}

/// Check the localizer's language and fallback accessors in a const context.
const fn localizer_fallback(localizer: &Localizer) -> bool {
    let _language = localizer.language();
    localizer.used_fallback()
}

/// Assert that the public const APIs work in an external crate context.
fn main() {
    assert_eq!(SPAN_START_LINE, 2);
    assert_eq!(SEGMENT_END, 5);
    assert_eq!(TRAIT_WARN, 25);
    assert_eq!(TRAIT_DENY, 35);
    assert_eq!(TRAIT_DEFAULT_CC_WARN, 45);
    assert_eq!(TYPE_DENY, 90);
    assert_eq!(TRAIT_ITEM_TOTAL, 0);
    assert_eq!(complexity_score(&COMPLEXITY), 0);

    let _ = AttributeKind::Inner.is_inner();
    let _ = trait_metrics_shape as fn(&TraitMetrics) -> usize;
    let _ = method_metrics_shape as fn(&MethodMetrics) -> usize;
    let _ = method_is_brain as fn(&MethodMetrics, usize, usize) -> bool;
    let _ = type_metrics_shape as fn(&TypeMetrics) -> usize;
    let _ = trait_diagnostic_shape as fn(&BrainTraitDiagnostic) -> usize;
    let _ = type_diagnostic_shape as fn(&BrainTypeDiagnostic) -> usize;
    let _ = attribute_shape as fn(&Attribute) -> bool;
    let _ = span_shape as fn(SourceSpan) -> usize;
    let _ = locale_fallback as fn(&LocaleSelection) -> bool;
    let _ = localizer_fallback as fn(&Localizer) -> bool;
}