//! Shared fixture builders for decomposition-advice unit tests.

use super::super::profile::{
    DecompositionContext, MethodProfile, MethodProfileBuilder, SubjectKind,
};
use super::super::{DecompositionSuggestion, SuggestedExtractionKind, suggest_decomposition};

pub(super) struct ExpectedSuggestion<'a> {
    pub(super) label: &'a str,
    pub(super) extraction_kind: SuggestedExtractionKind,
    pub(super) methods: &'a [&'a str],
}

pub(super) struct MethodInput<'a> {
    pub(super) name: &'a str,
    pub(super) fields: &'a [&'a str],
    pub(super) signature_types: &'a [&'a str],
    pub(super) local_types: &'a [&'a str],
    pub(super) domains: &'a [&'a str],
}

pub(super) fn assert_suggestion(
    actual: &DecompositionSuggestion,
    expected: ExpectedSuggestion<'_>,
) {
    assert_eq!(actual.label(), expected.label);
    assert_eq!(actual.extraction_kind(), expected.extraction_kind);
    assert_eq!(actual.methods(), expected.methods);
}

pub(super) fn profile(input: MethodInput<'_>) -> MethodProfile {
    let mut builder = MethodProfileBuilder::new(input.name);
    for field in input.fields {
        builder.record_accessed_field(*field);
    }
    for type_name in input.signature_types {
        builder.record_signature_type(*type_name);
    }
    for type_name in input.local_types {
        builder.record_local_type(*type_name);
    }
    for domain in input.domains {
        builder.record_external_domain(*domain);
    }
    builder.build()
}

pub(super) fn parser_serde_fs_fixture() -> Vec<MethodProfile> {
    vec![
        profile(MethodInput {
            name: "parse_tokens",
            fields: &["grammar", "tokens"],
            signature_types: &["TokenStream"],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "parse_nodes",
            fields: &["grammar", "ast"],
            signature_types: &[],
            local_types: &["ParseState"],
            domains: &[],
        }),
        profile(MethodInput {
            name: "encode_json",
            fields: &[],
            signature_types: &["Serializer"],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "decode_json",
            fields: &[],
            signature_types: &["Deserializer"],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "load_from_disk",
            fields: &[],
            signature_types: &[],
            local_types: &["PathBuf"],
            domains: &["std::fs"],
        }),
        profile(MethodInput {
            name: "save_to_disk",
            fields: &[],
            signature_types: &[],
            local_types: &["PathBuf"],
            domains: &["std::fs"],
        }),
    ]
}

pub(super) fn assert_type_decomposition_is_empty(subject: &str, methods: Vec<MethodProfile>) {
    let context = DecompositionContext::new(subject, SubjectKind::Type);
    assert!(
        suggest_decomposition(&context, &methods).is_empty(),
        "expected no decomposition suggestions for {subject}"
    );
}
