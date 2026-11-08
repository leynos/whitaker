//! Policy tests for continuation classification.

use super::{ContinuationDisposition, RequiredReason, classify};
use crate::facts::{CaptureStatus, LiteralBody, LiteralContext, LiteralFacts};

#[test]
fn ignores_literals_without_continuations() {
    let facts = LiteralFacts::Cooked {
        body: LiteralBody::new("ordinary text"),
        context: LiteralContext::Expression,
    };

    assert!(matches!(classify(&facts), ContinuationDisposition::Ignore));
}

#[test]
fn ignores_raw_and_generated_format_strings() {
    let raw = LiteralFacts::Raw;
    let generated = LiteralFacts::Cooked {
        body: LiteralBody::new("alpha\\\nbeta"),
        context: LiteralContext::GeneratedFormatString,
    };

    assert!(matches!(classify(&raw), ContinuationDisposition::Ignore));
    assert!(matches!(
        classify(&generated),
        ContinuationDisposition::Ignore
    ));
}

#[test]
fn requires_continuation_for_generated_source_files() {
    let facts = LiteralFacts::Cooked {
        body: LiteralBody::new("alpha\\\nbeta"),
        context: LiteralContext::GeneratedSourceFile,
    };

    assert!(matches!(
        classify(&facts),
        ContinuationDisposition::RequireContinuation(RequiredReason::GeneratedSourceFile)
    ));
}

#[test]
fn requires_continuation_for_non_string_literal_types() {
    assert!(matches!(
        classify(&LiteralFacts::NonStringLiteral),
        ContinuationDisposition::RequireContinuation(RequiredReason::NonStringLiteralType)
    ));
}

#[test]
fn prefers_a_proven_rewrite_for_expression_and_uncaptured_format_string() {
    let expression = LiteralFacts::Cooked {
        body: LiteralBody::new("alpha\\\nbeta"),
        context: LiteralContext::Expression,
    };
    let format_string = LiteralFacts::Cooked {
        body: LiteralBody::new("alpha\\\nbeta"),
        context: LiteralContext::SourceFormatString {
            capture: CaptureStatus::NoneObserved,
        },
    };

    assert!(matches!(
        classify(&expression),
        ContinuationDisposition::PreferConcat(_)
    ));
    assert!(matches!(
        classify(&format_string),
        ContinuationDisposition::PreferConcat(_)
    ));
}

#[test]
fn implicit_capture_withholds_the_rewrite() {
    let facts = LiteralFacts::Cooked {
        body: LiteralBody::new("alpha\\\nbeta"),
        context: LiteralContext::SourceFormatString {
            capture: CaptureStatus::Present,
        },
    };

    assert!(matches!(
        classify(&facts),
        ContinuationDisposition::RequireContinuation(RequiredReason::ImplicitFormatCapture)
    ));
}

#[rstest::rstest]
#[case(" \\\nbeta")]
#[case("alpha\\\n \n")]
fn layout_only_continuations_require_the_original_spelling(#[case] body: &str) {
    let facts = LiteralFacts::Cooked {
        body: LiteralBody::new(body),
        context: LiteralContext::Expression,
    };

    assert!(matches!(
        classify(&facts),
        ContinuationDisposition::RequireContinuation(RequiredReason::LeadingOrTrailingLayoutTrim)
    ));
}

#[test]
fn a_join_can_be_rewritten_while_layout_trim_stays_inside_a_fragment() {
    let body = LiteralBody::new("\\\n alpha\\\n beta");
    let facts = LiteralFacts::Cooked {
        body,
        context: LiteralContext::Expression,
    };

    let ContinuationDisposition::PreferConcat(rewrite) = classify(&facts) else {
        panic!("the interior join must still be rewritable");
    };
    assert!(
        rewrite
            .fragments()
            .first()
            .is_some_and(|fragment| fragment.starts_with("\\\n")),
        "the leading layout continuation must remain inside the first fragment"
    );
}
