//! Unit tests for decomposition diagnostic-note rendering.

use super::format_diagnostic_note;
use crate::decomposition_advice::{
    DecompositionContext, MethodProfile, MethodProfileBuilder, SubjectKind, suggest_decomposition,
};

struct MethodInput<'a> {
    name: &'a str,
    fields: &'a [&'a str],
    signature_types: &'a [&'a str],
    local_types: &'a [&'a str],
    domains: &'a [&'a str],
}

fn profile(input: MethodInput<'_>) -> MethodProfile {
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

fn parser_serde_fs_suggestions() -> (
    DecompositionContext,
    Vec<crate::decomposition_advice::DecompositionSuggestion>,
) {
    let context = DecompositionContext::new("Foo", SubjectKind::Type);
    let methods = vec![
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
    ];
    let suggestions = suggest_decomposition(&context, &methods);
    (context, suggestions)
}

#[test]
fn format_diagnostic_note_returns_none_for_empty_suggestions() {
    let context = DecompositionContext::new("Foo", SubjectKind::Type);
    assert_eq!(format_diagnostic_note(&context, &[]), None);
}

#[test]
fn format_diagnostic_note_renders_type_suggestions() {
    let (context, suggestions) = parser_serde_fs_suggestions();
    let rendered = format_diagnostic_note(&context, &suggestions).unwrap_or_default();

    assert_eq!(
        rendered,
        concat!(
            "Potential decomposition for `Foo`:\n",
            "- [grammar] helper struct for `parse_nodes`, `parse_tokens`\n",
            "- [serde::json] module for `decode_json`, `encode_json`\n",
            "- [std::fs] module for `load_from_disk`, `save_to_disk`",
        ),
    );
}

#[test]
fn format_diagnostic_note_renders_trait_sub_traits() {
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

    let rendered = format_diagnostic_note(&context, &suggest_decomposition(&context, &methods))
        .unwrap_or_default();

    assert!(rendered.contains("- [serde::json] sub-trait for `decode_request`, `encode_request`"));
    assert!(rendered.contains("- [std::io] sub-trait for `read_frame`, `write_frame`"));
}

#[test]
fn format_diagnostic_note_caps_rendered_suggestions() {
    let context = DecompositionContext::new("Coordinator", SubjectKind::Type);
    let methods = vec![
        profile(MethodInput {
            name: "grammar_alpha",
            fields: &["grammar"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "grammar_beta",
            fields: &["grammar"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "serde_alpha",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "serde_beta",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["serde::json"],
        }),
        profile(MethodInput {
            name: "io_alpha",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["std::io"],
        }),
        profile(MethodInput {
            name: "io_beta",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["std::io"],
        }),
        profile(MethodInput {
            name: "fs_alpha",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["std::fs"],
        }),
        profile(MethodInput {
            name: "fs_beta",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["std::fs"],
        }),
    ];
    let suggestions = suggest_decomposition(&context, &methods);

    let rendered = format_diagnostic_note(&context, &suggestions).unwrap_or_default();

    assert!(rendered.contains("- [grammar] helper struct"));
    assert!(rendered.contains("- [serde::json] module"));
    assert!(rendered.contains("- [std::fs] module"));
    assert!(rendered.contains("1 more areas omitted"));
    assert!(!rendered.contains("[std::io]"));
}

#[test]
fn format_diagnostic_note_caps_methods_per_suggestion() {
    let context = DecompositionContext::new("Reporter", SubjectKind::Type);
    let methods = vec![
        profile(MethodInput {
            name: "report_alpha",
            fields: &["report"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "report_beta",
            fields: &["report"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "report_delta",
            fields: &["report"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "report_epsilon",
            fields: &["report"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "report_gamma",
            fields: &["report"],
            signature_types: &[],
            local_types: &[],
            domains: &[],
        }),
        profile(MethodInput {
            name: "io_alpha",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["std::io"],
        }),
        profile(MethodInput {
            name: "io_beta",
            fields: &[],
            signature_types: &[],
            local_types: &[],
            domains: &["std::io"],
        }),
    ];
    let suggestions = suggest_decomposition(&context, &methods);

    let rendered = format_diagnostic_note(&context, &suggestions).unwrap_or_default();

    assert!(rendered.contains(
        "- [report] helper struct for `report_alpha`, `report_beta`, `report_delta`, +2 more methods"
    ));
}
