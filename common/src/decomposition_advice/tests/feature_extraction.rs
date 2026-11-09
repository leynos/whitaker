//! Unit tests for identifier keywords and feature-vector construction.

use std::str::FromStr;

use super::{
    super::{
        suggestion::SuggestedExtractionKind,
        vector::{build_feature_vector, dot_product, identifier_keywords},
    },
    test_fixtures::{MethodInput, profile},
};

/// Split camel-case identifiers and discard stop words.
#[test]
fn identifier_keywords_split_camel_case_and_remove_stop_words() {
    let keywords = identifier_keywords("buildRenderTree");
    assert_eq!(keywords, ["tree"]);
}

/// Accept both documented spellings of the sub-trait extraction kind.
#[test]
fn suggested_extraction_kind_accepts_both_sub_trait_spellings() {
    assert_eq!(
        SuggestedExtractionKind::from_str("sub trait"),
        Ok(SuggestedExtractionKind::SubTrait)
    );
    assert_eq!(
        SuggestedExtractionKind::from_str("sub-trait"),
        Ok(SuggestedExtractionKind::SubTrait)
    );
    assert_eq!(SuggestedExtractionKind::SubTrait.to_string(), "sub-trait");
}

/// Split acronym boundaries in mixed-case identifiers.
#[test]
fn identifier_keywords_handle_acronyms_and_mixed_case() {
    assert_eq!(identifier_keywords("HTTPRequest"), ["http", "request"]);
    assert_eq!(
        identifier_keywords("XMLHttpRequest"),
        ["xml", "http", "request"]
    );
}

/// Split identifiers at non-alphanumeric separators.
#[test]
fn identifier_keywords_split_on_non_alphanumeric_separators() {
    assert_eq!(identifier_keywords("build_render_tree"), ["tree"]);
    assert_eq!(identifier_keywords("build-render-tree"), ["tree"]);
}

/// Remove stop words and handle empty identifiers.
#[test]
fn identifier_keywords_only_stop_words_or_empty_input_yield_empty() {
    assert!(identifier_keywords("build_render").is_empty());
    assert!(identifier_keywords("").is_empty());
}

/// Prefix feature keys with their category names.
#[test]
fn feature_vector_prefixes_categories() {
    let vector = build_feature_vector(&profile(MethodInput {
        name: "state",
        fields: &["state"],
        signature_types: &[],
        local_types: &[],
        domains: &[],
    }));

    assert!(vector.weights().contains_key("field:state"));
    assert!(vector.weights().contains_key("keyword:state"));
}

/// Return zero for feature vectors with disjoint keys.
#[test]
fn dot_product_is_zero_for_disjoint_profiles() {
    let left = build_feature_vector(&profile(MethodInput {
        name: "parse",
        fields: &["grammar"],
        signature_types: &[],
        local_types: &[],
        domains: &[],
    }));
    let right = build_feature_vector(&profile(MethodInput {
        name: "write",
        fields: &[],
        signature_types: &[],
        local_types: &[],
        domains: &["std::fs"],
    }));

    assert_eq!(dot_product(left.weights(), right.weights()), 0);
}
