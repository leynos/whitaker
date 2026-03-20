//! Unit tests for decomposition diagnostic-note rendering.

use super::format_diagnostic_note;
use crate::decomposition_advice::{DecompositionContext, MethodProfile, SubjectKind};
use crate::test_support::decomposition::{
    MethodInput, decomposition_suggestions, parser_serde_fs_fixture, profile,
    transport_trait_fixture,
};

fn parser_serde_fs_suggestions() -> (
    DecompositionContext,
    Vec<crate::decomposition_advice::DecompositionSuggestion>,
) {
    decomposition_suggestions("Foo", SubjectKind::Type, &parser_serde_fs_fixture())
}

fn render_note(subject: &str, kind: SubjectKind, methods: Vec<MethodProfile>) -> String {
    let (context, suggestions) = decomposition_suggestions(subject, kind, &methods);
    format_diagnostic_note(&context, &suggestions).unwrap_or_default()
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
    let rendered = render_note("Transport", SubjectKind::Trait, transport_trait_fixture());

    assert!(rendered.contains("- [serde::json] sub-trait for `decode_request`, `encode_request`"));
    assert!(rendered.contains("- [std::io] sub-trait for `read_frame`, `write_frame`"));
}

#[test]
fn format_diagnostic_note_caps_rendered_suggestions() {
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

    let rendered = render_note("Coordinator", SubjectKind::Type, methods);

    assert!(rendered.contains("- [grammar] helper struct"));
    assert!(rendered.contains("- [serde::json] module"));
    assert!(rendered.contains("- [std::fs] module"));
    assert!(rendered.contains("1 more area omitted"));
    assert!(!rendered.contains("[std::io]"));
}

#[test]
fn format_diagnostic_note_caps_methods_per_suggestion() {
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

    let rendered = render_note("Reporter", SubjectKind::Type, methods);

    assert!(rendered.contains(
        "- [report] helper struct for `report_alpha`, `report_beta`, `report_delta`, +2 more methods"
    ));
}
