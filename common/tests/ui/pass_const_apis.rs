//! Compile-pass checks for const public common APIs in an external crate.

use whitaker_common::complexity_signal::{LineSegment, SegmentError};
use whitaker_common::span::{SourceLocation, SourceSpan};
use whitaker_common::{
    BrainTraitThresholdsBuilder, BrainTypeThresholdsBuilder, CognitiveComplexityBuilder,
    LocaleSelection, Localizer,
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
const TYPE_DENY: usize = BrainTypeThresholdsBuilder::new()
    .wmc_warn(40)
    .wmc_deny(90)
    .lcom4_warn(2)
    .lcom4_deny(4)
    .brain_method_deny_count(3)
    .build()
    .wmc_deny();
const COMPLEXITY: CognitiveComplexityBuilder = CognitiveComplexityBuilder::new();

/// Read the builder score in a const context.
const fn complexity_score(builder: &CognitiveComplexityBuilder) -> usize {
    builder.score()
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
    assert_eq!(TYPE_DENY, 90);
    assert_eq!(complexity_score(&COMPLEXITY), 0);
    let _ = locale_fallback as fn(&LocaleSelection) -> bool;
    let _ = localizer_fallback as fn(&Localizer) -> bool;
}
