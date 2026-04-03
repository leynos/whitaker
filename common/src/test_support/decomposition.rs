//! Shared decomposition-advice fixtures for unit and integration tests.
//!
//! These helpers keep the recurring grammar/serde/filesystem and transport
//! method communities aligned across diagnostic unit tests and behaviour
//! coverage.

#[path = "decomposition_adjacency.rs"]
mod adjacency;
#[path = "decomposition_vector_algebra.rs"]
mod vector_algebra;

use crate::decomposition_advice::{
    DecompositionContext, DecompositionSuggestion, MethodProfile, MethodProfileBuilder,
    SubjectKind, methods_meet_cosine_threshold as runtime_methods_meet_cosine_threshold,
    suggest_decomposition,
};

pub use self::adjacency::{AdjacencyReport, EdgeInput, adjacency_report};
pub use self::vector_algebra::{MethodVectorAlgebraReport, method_vector_algebra};

/// Input data for building a [`MethodProfile`] in tests.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::test_support::decomposition::{MethodInput, profile};
///
/// let profile = profile(MethodInput {
///     name: "parse_tokens",
///     fields: &["grammar"],
///     signature_types: &[],
///     local_types: &[],
///     domains: &[],
/// });
///
/// assert_eq!(profile.name(), "parse_tokens");
/// ```
pub struct MethodInput<'a> {
    pub name: &'a str,
    pub fields: &'a [&'a str],
    pub signature_types: &'a [&'a str],
    pub local_types: &'a [&'a str],
    pub domains: &'a [&'a str],
}

/// Builds a [`MethodProfile`] from declarative test input.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::test_support::decomposition::{MethodInput, profile};
///
/// let profile = profile(MethodInput {
///     name: "save_to_disk",
///     fields: &[],
///     signature_types: &[],
///     local_types: &["PathBuf"],
///     domains: &["std::fs"],
/// });
///
/// assert_eq!(profile.name(), "save_to_disk");
/// ```
#[must_use]
pub fn profile(input: MethodInput<'_>) -> MethodProfile {
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

/// Builds the recurring parser/serde/filesystem fixture.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::test_support::decomposition::parser_serde_fs_fixture;
///
/// let methods = parser_serde_fs_fixture();
/// assert_eq!(methods.len(), 6);
/// ```
#[must_use]
pub fn parser_serde_fs_fixture() -> Vec<MethodProfile> {
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

/// Builds the recurring transport trait fixture.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::test_support::decomposition::transport_trait_fixture;
///
/// let methods = transport_trait_fixture();
/// assert_eq!(methods.len(), 4);
/// ```
#[must_use]
pub fn transport_trait_fixture() -> Vec<MethodProfile> {
    vec![
        profile(MethodInput {
            name: "encode_request",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "decode_request",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "read_frame",
            fields: &[],
            signature_types: &["IoBuffer"],
            local_types: &[],
            domains: &["std::io"],
        }),
        profile(MethodInput {
            name: "write_frame",
            fields: &[],
            signature_types: &["IoBuffer"],
            local_types: &[],
            domains: &["std::io"],
        }),
    ]
}

/// Computes decomposition suggestions for `methods`.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::decomposition_advice::SubjectKind;
/// use whitaker_common::test_support::decomposition::{
///     decomposition_suggestions,
///     parser_serde_fs_fixture,
/// };
///
/// let methods = parser_serde_fs_fixture();
/// let (_context, suggestions) =
///     decomposition_suggestions("Foo", SubjectKind::Type, &methods);
/// assert!(!suggestions.is_empty());
/// ```
#[must_use]
pub fn decomposition_suggestions(
    subject: &str,
    kind: SubjectKind,
    methods: &[MethodProfile],
) -> (DecompositionContext, Vec<DecompositionSuggestion>) {
    let context = DecompositionContext::new(subject, kind);
    let suggestions = suggest_decomposition(&context, methods);
    (context, suggestions)
}

/// Evaluates whether two methods satisfy Whitaker's cosine threshold.
///
/// This helper exists for behaviour tests that need an observable seam without
/// widening the production decomposition API.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::test_support::decomposition::{MethodInput, methods_meet_cosine_threshold, profile};
///
/// let left = profile(MethodInput {
///     name: "parse_tokens",
///     fields: &["grammar"],
///     signature_types: &[],
///     local_types: &[],
///     domains: &[],
/// });
/// let right = profile(MethodInput {
///     name: "parse_nodes",
///     fields: &["grammar"],
///     signature_types: &[],
///     local_types: &[],
///     domains: &[],
/// });
///
/// assert!(methods_meet_cosine_threshold(&left, &right));
/// ```
#[must_use]
pub fn methods_meet_cosine_threshold(left: &MethodProfile, right: &MethodProfile) -> bool {
    runtime_methods_meet_cosine_threshold(left, right)
}
