//! Shared fixture builders for decomposition-advice unit tests.

use super::super::profile::{DecompositionContext, MethodProfile, SubjectKind};
use super::super::{DecompositionSuggestion, SuggestedExtractionKind, suggest_decomposition};
pub(super) use crate::test_support::decomposition::{
    MethodInput, parser_serde_fs_fixture, profile,
};

pub(super) struct ExpectedSuggestion<'a> {
    pub(super) label: &'a str,
    pub(super) extraction_kind: SuggestedExtractionKind,
    pub(super) methods: &'a [&'a str],
}

pub(super) fn assert_suggestion(
    actual: &DecompositionSuggestion,
    expected: ExpectedSuggestion<'_>,
) {
    assert_eq!(actual.label(), expected.label);
    assert_eq!(actual.extraction_kind(), expected.extraction_kind);
    assert_eq!(actual.methods(), expected.methods);
}

pub(super) fn assert_type_decomposition_is_empty(subject: &str, methods: Vec<MethodProfile>) {
    let context = DecompositionContext::new(subject, SubjectKind::Type);
    assert!(
        suggest_decomposition(&context, &methods).is_empty(),
        "expected no decomposition suggestions for {subject}"
    );
}
