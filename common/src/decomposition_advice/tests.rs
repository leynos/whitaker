//! Unit tests covering decomposition feature extraction and clustering.

mod test_fixtures;

use self::test_fixtures::{
    ExpectedSuggestion, MethodInput, assert_suggestion, assert_type_decomposition_is_empty,
    parser_serde_fs_fixture, profile,
};
use super::community::{build_similarity_edges, detect_communities};
use super::profile::{DecompositionContext, SubjectKind};
use super::suggestion::{SuggestedExtractionKind, suggest_decomposition};
use super::vector::{build_feature_vector, dot_product, identifier_keywords};
use std::str::FromStr;

#[test]
fn identifier_keywords_split_camel_case_and_remove_stop_words() {
    let keywords = identifier_keywords("buildRenderTree");
    assert_eq!(keywords, ["tree"]);
}

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

#[test]
fn identifier_keywords_handle_acronyms_and_mixed_case() {
    assert_eq!(identifier_keywords("HTTPRequest"), ["http", "request"]);
    assert_eq!(
        identifier_keywords("XMLHttpRequest"),
        ["xml", "http", "request"]
    );
}

#[test]
fn identifier_keywords_split_on_non_alphanumeric_separators() {
    assert_eq!(identifier_keywords("build_render_tree"), ["tree"]);
    assert_eq!(identifier_keywords("build-render-tree"), ["tree"]);
}

#[test]
fn identifier_keywords_only_stop_words_or_empty_input_yield_empty() {
    assert!(identifier_keywords("build_render").is_empty());
    assert!(identifier_keywords("").is_empty());
}

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

#[test]
fn similarity_edges_include_related_methods_only() {
    let vectors = vec![
        build_feature_vector(&profile(MethodInput {
            name: "parse_tokens",
            fields: &["grammar"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        })),
        build_feature_vector(&profile(MethodInput {
            name: "parse_nodes",
            fields: &["grammar"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        })),
        build_feature_vector(&profile(MethodInput {
            name: "write_file",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["std::fs"],
        })),
    ];

    let edges = build_similarity_edges(&vectors);

    assert_eq!(edges.len(), 1);
    assert_eq!((edges[0].left(), edges[0].right()), (0, 1));
    assert!(edges[0].weight() > 0);
}

#[test]
fn detect_communities_is_order_invariant() {
    let fixture = parser_serde_fs_fixture();
    let mut original_vectors: Vec<_> = fixture.iter().map(build_feature_vector).collect();
    original_vectors.sort();

    let reordered_fixture = [
        fixture[4].clone(),
        fixture[1].clone(),
        fixture[5].clone(),
        fixture[0].clone(),
        fixture[3].clone(),
        fixture[2].clone(),
    ];
    let mut reordered_vectors: Vec<_> =
        reordered_fixture.iter().map(build_feature_vector).collect();
    reordered_vectors.sort();

    assert_eq!(
        detect_communities(&original_vectors),
        detect_communities(&reordered_vectors)
    );
}

#[test]
fn suggest_decomposition_returns_empty_for_single_community() {
    assert_type_decomposition_is_empty(
        "Parser",
        vec![
            profile(MethodInput {
                name: "parse_tokens",
                fields: &["grammar"],
                signature_types: &[],
                local_types: &[],
                domains: &[],
            }),
            profile(MethodInput {
                name: "parse_nodes",
                fields: &["grammar"],
                signature_types: &[],
                local_types: &[],
                domains: &[],
            }),
            profile(MethodInput {
                name: "parse_tree",
                fields: &["grammar"],
                signature_types: &[],
                local_types: &[],
                domains: &[],
            }),
            profile(MethodInput {
                name: "parse_stream",
                fields: &["grammar"],
                signature_types: &[],
                local_types: &[],
                domains: &[],
            }),
        ],
    );
}

#[test]
fn suggest_decomposition_for_type_prefers_domain_module_and_field_helper_struct() {
    let context = DecompositionContext::new("Foo", SubjectKind::Type);
    let suggestions = suggest_decomposition(&context, &parser_serde_fs_fixture());

    assert_eq!(suggestions.len(), 3);
    assert_suggestion(
        &suggestions[0],
        ExpectedSuggestion {
            label: "grammar",
            extraction_kind: SuggestedExtractionKind::HelperStruct,
            methods: &["parse_nodes", "parse_tokens"],
        },
    );
    assert_suggestion(
        &suggestions[1],
        ExpectedSuggestion {
            label: "serde::json",
            extraction_kind: SuggestedExtractionKind::Module,
            methods: &["decode_json", "encode_json"],
        },
    );
    assert_suggestion(
        &suggestions[2],
        ExpectedSuggestion {
            label: "std::fs",
            extraction_kind: SuggestedExtractionKind::Module,
            methods: &["load_from_disk", "save_to_disk"],
        },
    );
}

#[test]
fn suggest_decomposition_for_trait_returns_sub_trait_suggestions() {
    let context = DecompositionContext::new("Transport", SubjectKind::Trait);
    let methods = vec![
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
    ];

    let suggestions = suggest_decomposition(&context, &methods);

    assert_eq!(suggestions.len(), 2);
    assert!(
        suggestions.iter().all(|suggestion| {
            suggestion.extraction_kind() == SuggestedExtractionKind::SubTrait
        })
    );
}

#[test]
fn suggest_decomposition_is_order_invariant_for_duplicate_method_names() {
    let context = DecompositionContext::new("Importer", SubjectKind::Type);
    let methods = vec![
        profile(MethodInput {
            name: "load",
            fields: &[],
            signature_types: &["JsonDecoder"],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "load",
            fields: &[],
            signature_types: &["JsonReader"],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "load",
            fields: &["cache"],
            signature_types: &[],
            local_types: &["PathBuf"],
            domains: &["std::fs"],
        }),
        profile(MethodInput {
            name: "load",
            fields: &["cache"],
            signature_types: &[],
            local_types: &["PathBuf"],
            domains: &["std::fs"],
        }),
    ];

    let reordered = vec![
        methods[2].clone(),
        methods[0].clone(),
        methods[3].clone(),
        methods[1].clone(),
    ];

    assert_eq!(
        suggest_decomposition(&context, &methods),
        suggest_decomposition(&context, &reordered)
    );
}

#[test]
fn suggestions_drop_singleton_noise_methods() {
    let context = DecompositionContext::new("Foo", SubjectKind::Type);
    let mut methods = parser_serde_fs_fixture();
    methods.push(profile(MethodInput {
        name: "run",
        fields: &[],
        signature_types: &[],
        local_types: &[],
        domains: &[],
    }));

    let suggestions = suggest_decomposition(&context, &methods);

    assert_eq!(suggestions.len(), 3);
    assert!(
        suggestions
            .iter()
            .all(|suggestion| !suggestion.methods().contains(&String::from("run")))
    );
}

#[test]
fn suggestions_skip_degenerate_groups_without_features() {
    assert_type_decomposition_is_empty(
        "Runner",
        vec![
            profile(MethodInput {
                name: "build",
                fields: &[],
                signature_types: &[],
                local_types: &[],
                domains: &[],
            }),
            profile(MethodInput {
                name: "make",
                fields: &[],
                signature_types: &[],
                local_types: &[],
                domains: &[],
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
        ],
    );
}
