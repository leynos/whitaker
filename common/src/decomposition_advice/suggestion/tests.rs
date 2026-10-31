//! Tests for all-or-nothing suggestion construction on malformed state.

use super::{build_suggestion, build_suggestions};
use crate::decomposition_advice::{
    DecompositionContext, MethodProfileBuilder, SubjectKind,
    error::DecompositionError,
    vector::{build_feature_vector, minimal_feature_vector, test_feature_vector},
};

/// Construct a feature vector with internally consistent metadata.
fn valid_vector() -> crate::decomposition_advice::vector::MethodFeatureVector {
    let mut profile = MethodProfileBuilder::new("parse_config");
    profile.record_accessed_field("grammar");
    build_feature_vector(&profile.build())
}

/// Return a representative type context for suggestion rendering.
fn context() -> DecompositionContext {
    DecompositionContext::new("Parser", SubjectKind::Type)
}

/// Reject a community containing an index outside the feature-vector set.
#[test]
fn build_suggestion_rejects_invalid_community_index() {
    let vectors = [minimal_feature_vector("parse_config")];

    let result = build_suggestion(&context(), &[0, 1], &vectors);

    assert!(matches!(
        result,
        Err(DecompositionError::InvalidCommunityIndex {
            vector_count: 1,
            community_count: 2,
        })
    ));
}

/// Reject weighted features that have no display metadata.
#[test]
fn build_suggestion_rejects_missing_feature_metadata() {
    let vectors = [test_feature_vector("parse_config", &[("field:grammar", 6)])];

    let result = build_suggestion(&context(), &[0], &vectors);

    assert!(matches!(
        result,
        Err(DecompositionError::MissingFeatureMetadata { vector_count: 1 })
    ));
}

/// Return no partial suggestions when a later community is malformed.
#[test]
fn build_suggestions_discards_earlier_valid_communities_on_error() {
    let vectors = [
        valid_vector(),
        test_feature_vector("write_file", &[("domain:std::fs", 5)]),
    ];
    let communities = [vec![0], vec![1]];

    let result = build_suggestions(&context(), &communities, &vectors);

    assert!(matches!(
        result,
        Err(DecompositionError::MissingFeatureMetadata { vector_count: 2 })
    ));
}
