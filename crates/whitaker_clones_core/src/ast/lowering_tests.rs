//! Adapter-scoped tests for Rust syntax lowering.

use insta::assert_json_snapshot;
use rstest::rstest;
use serde_json::json;

use ra_ap_syntax::{AstNode, Edition, SourceFile};

use super::{
    LoweringLimits, MAX_AST_DEPTH, MAX_AST_NODES, kind_id, leaf_class,
    validate_covering_node_budget,
};
use crate::{
    AstError, ByteSpan, Production,
    ast::{KindId, LeafClass, NormalizedNode, NormalizedTree, PARSER_SCHEMA_VERSION},
    canonical_hash, kind_counts, lower_span, production_multiset,
};

fn kind_name(kind: KindId) -> String {
    let parser_kind = ra_ap_syntax::SyntaxKind::from(kind.get());
    format!("{parser_kind:?}")
}

#[rstest]
fn pinned_parser_snapshot_parses_current_edition_source() {
    let tree = lower_span_for("fn f() {}", "fn f").expect("source should lower");
    assert_eq!(kind_name(tree.root().kind()), "SOURCE_FILE");
}

#[rstest]
fn exact_node_span_selects_that_node() {
    let tree = lower_span_for("fn f() { let sum = a + b; }", "a + b")
        .expect("binary expression should lower");
    assert_eq!(kind_name(tree.root().kind()), "BIN_EXPR");
}

#[rstest]
fn smallest_inner_expression_span_does_not_select_the_file() {
    let tree = lower_span_for("fn f() { call(value); }", "value").expect("identifier should lower");
    assert_ne!(kind_name(tree.root().kind()), "SOURCE_FILE");
    assert!(contains_leaf_class(tree.root(), LeafClass::Ident));
}

#[rstest]
fn two_sibling_span_selects_common_expression_ancestor() {
    let tree = lower_span_for("fn f() { let value = a + b; }", "a + b").expect("span should lower");
    assert_eq!(kind_name(tree.root().kind()), "BIN_EXPR");
    assert!(contains_leaf_class(tree.root(), LeafClass::Other));
}

#[rstest]
fn whole_file_span_selects_source_file() -> Result<(), AstError> {
    let source = "fn f() {}";
    let span = ByteSpan::new(source, 0, source.len() as u32)?;
    let tree = lower_span(source, span)?;
    assert_eq!(kind_name(tree.root().kind()), "SOURCE_FILE");
    Ok(())
}

#[rstest]
fn large_synthetic_source_still_lowers() -> Result<(), AstError> {
    let statements = (0..600)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!("fn generated() {{ {statements} }}");
    let span = ByteSpan::new(&source, 0, source.len() as u32)?;
    let tree = lower_span(&source, span)?;
    assert_eq!(kind_name(tree.root().kind()), "SOURCE_FILE");
    Ok(())
}

#[rstest]
fn oversized_source_is_rejected_by_the_node_budget() -> Result<(), AstError> {
    let statements = (0..=MAX_AST_NODES)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!("fn generated() {{ {statements} }}");
    let span = ByteSpan::new(&source, 0, source.len() as u32)?;

    assert_eq!(
        lower_span(&source, span),
        Err(AstError::TreeTooLarge {
            limit: MAX_AST_NODES
        })
    );
    Ok(())
}

#[rstest]
fn deeply_nested_syntax_obeys_the_lowering_depth_budget() -> Result<(), AstError> {
    let source = "fn f() { if true { if true { if true { if true { 0; } } } } }";
    let root = SourceFile::parse(source, Edition::CURRENT)
        .tree()
        .syntax()
        .clone();
    let span = ByteSpan::new(source, 0, source.len() as u32)?;

    assert_eq!(
        LoweringLimits::with_depth_limit(2, span).lower(&root, 0),
        Err(AstError::TreeTooDeep { limit: 2 })
    );
    Ok(())
}

#[rstest]
fn covering_node_selection_budget_surfaces_typed_errors() -> Result<(), AstError> {
    // The selection budget guards the covering-node walk independently of the
    // lowering budget, and both breaches must surface as the same typed errors.
    let span = ByteSpan::new("fn f() {}", 0, 2)?;
    assert_eq!(
        validate_covering_node_budget(span, MAX_AST_DEPTH + 1, 0),
        Err(AstError::TreeTooDeep {
            limit: MAX_AST_DEPTH
        })
    );
    assert_eq!(
        validate_covering_node_budget(span, 0, MAX_AST_NODES),
        Err(AstError::TreeTooLarge {
            limit: MAX_AST_NODES
        })
    );
    Ok(())
}

#[rstest]
fn small_candidate_amid_unrelated_nodes_is_not_rejected_by_the_budget() -> Result<(), AstError> {
    // A tiny valid candidate (`a + b`) buried in a function whose remaining
    // statements far exceed the node budget. Pruned covering-node selection must
    // descend only the ancestor chain, so the unrelated statements neither count
    // toward the budget nor prevent the candidate from lowering.
    let filler = (0..=MAX_AST_NODES)
        .map(|index| format!("let filler_{index} = {index};"))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!("fn generated() {{ let target = a + b; {filler} }}");

    let tree = lower_span_for(&source, "a + b").expect("small candidate should lower");
    assert_eq!(kind_name(tree.root().kind()), "BIN_EXPR");
    Ok(())
}

#[rstest]
fn literal_tokens_lower_as_normalized_literal_leaves() {
    let tree = lower_span_for("fn f() { let value = 42; }", "42").expect("literal should lower");
    assert!(contains_leaf_class(tree.root(), LeafClass::Literal));
}

#[rstest]
#[case::empty(ByteSpan::new("fn f() {}", 2, 2), AstError::EmptySpan { offset: 2 })]
#[case::inverted(
    ByteSpan::new("fn f() {}", 4, 2),
    AstError::InvalidSpan { start: 4, end: 2 }
)]
#[case::out_of_bounds(
    ByteSpan::new("fn f() {}", 0, 40),
    AstError::SpanOutOfBounds { start: 0, end: 40, len: 9 }
)]
fn span_validation_reports_specific_errors(
    #[case] actual: Result<ByteSpan, AstError>,
    #[case] expected: AstError,
) {
    assert_eq!(actual, Err(expected));
}

#[rstest]
fn source_mismatch_non_char_boundary_is_reported_by_lowering() -> Result<(), AstError> {
    let span = ByteSpan::new("ab", 0, 1)?;
    assert_eq!(
        lower_span("é", span),
        Err(AstError::NonCharBoundary { offset: 1 })
    );
    Ok(())
}

#[rstest]
fn source_mismatch_out_of_bounds_is_reported_by_lowering() -> Result<(), AstError> {
    let span = ByteSpan::new("longer", 0, 6)?;
    assert_eq!(
        lower_span("short", span),
        Err(AstError::SpanOutOfBounds {
            start: 0,
            end: 6,
            len: 5
        })
    );
    Ok(())
}

#[rstest]
fn error_subtree_is_rejected() -> Result<(), AstError> {
    let source = "@error@";
    let span = ByteSpan::new(source, 0, source.len() as u32)?;
    assert_eq!(
        lower_span(source, span),
        Err(AstError::UnparsableSpan {
            start: 0,
            end: source.len() as u32
        })
    );
    Ok(())
}

fn lower_span_for(source: &str, needle: &str) -> Result<NormalizedTree, AstError> {
    let start = source
        .find(needle)
        .ok_or(AstError::UnparsableSpan { start: 0, end: 0 })?;
    let end = start + needle.len();
    lower_span(source, ByteSpan::new(source, start as u32, end as u32)?)
}

fn contains_leaf_class(node: &NormalizedNode, leaf_class: LeafClass) -> bool {
    node.leaf() == Some(leaf_class)
        || node
            .children()
            .iter()
            .any(|child| contains_leaf_class(child, leaf_class))
}

#[rstest]
#[case::identifier(ra_ap_syntax::SyntaxKind::IDENT, LeafClass::Ident)]
#[case::lifetime(ra_ap_syntax::SyntaxKind::LIFETIME_IDENT, LeafClass::Ident)]
#[case::literal(ra_ap_syntax::SyntaxKind::INT_NUMBER, LeafClass::Literal)]
#[case::operator(ra_ap_syntax::SyntaxKind::PLUS, LeafClass::Other)]
fn token_leaf_class_is_stable(#[case] kind: ra_ap_syntax::SyntaxKind, #[case] leaf: LeafClass) {
    let node = NormalizedNode::new(kind_id(kind), Some(leaf_class(kind)), Vec::new());
    assert_eq!(node.leaf(), Some(leaf));
}

#[rstest]
fn kind_names_are_available_for_adapter_snapshots() {
    assert_eq!(kind_name(KindId::new(0)), "TOMBSTONE");
}

#[rstest]
fn ast_feature_vector_snapshot() -> Result<(), AstError> {
    let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let span = ByteSpan::new(source, 0, source.len() as u32)?;
    let tree = lower_span(source, span)?;
    let counts = kind_counts(&tree)
        .iter()
        .map(|(kind, depth, count)| {
            json!({
                "kind": kind_name(kind),
                "depth": depth.get(),
                "count": count,
            })
        })
        .collect::<Vec<_>>();
    let productions = production_multiset(&tree)
        .iter()
        .map(|(production, count)| {
            json!({
                "production": production_name(production),
                "count": count,
            })
        })
        .collect::<Vec<_>>();

    assert_json_snapshot!(
        "ast_feature_vector_add_function",
        json!({
            "schema": PARSER_SCHEMA_VERSION,
            "span": { "start": span.start(), "end": span.end() },
            "kind_counts": counts,
            "productions": productions,
            "canonical_hash": canonical_hash(&tree).to_hex(),
        })
    );
    Ok(())
}

#[rstest]
fn parser_schema_version_snapshot() {
    assert_json_snapshot!(
        "ast_parser_schema_version",
        json!({ "parser_schema_version": PARSER_SCHEMA_VERSION })
    );
}

fn production_name(production: Production) -> String {
    match production {
        Production::Bigram(parent, child) => {
            format!("{} -> {}", kind_name(parent), kind_name(child))
        }
        Production::Trigram(grandparent, parent, child) => format!(
            "{} -> {} -> {}",
            kind_name(grandparent),
            kind_name(parent),
            kind_name(child)
        ),
    }
}
