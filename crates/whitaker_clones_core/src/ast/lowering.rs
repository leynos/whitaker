//! Adapter from parser syntax trees into the parser-agnostic AST domain.

use std::ops::Range;

use ra_ap_syntax::{
    AstNode, Edition, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken, TextRange,
    TextSize,
};
use tracing::warn;

use super::{
    AstError, AstResult, ByteSpan, KindId, LeafClass, NormalisedNode, NormalisedTree,
    select_smallest_covering,
};

pub use crate::hashing::PARSER_SCHEMA_VERSION;

/// Parses `file_text`, maps `span` to the smallest covering syntax node, and
/// lowers that subtree into a [`NormalisedTree`].
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{ByteSpan, lower_span};
///
/// let span = ByteSpan::new("fn f() {}", 0, 2)?;
/// let tree = lower_span("fn f() {}", span)?;
///
/// assert_eq!(tree.span(), span);
/// # Ok::<(), whitaker_clones_core::AstError>(())
/// ```
pub fn lower_span(file_text: &str, span: ByteSpan) -> AstResult<NormalisedTree> {
    let span = ByteSpan::new(file_text, span.start(), span.end())?;
    let parse = SourceFile::parse(file_text, Edition::CURRENT);
    let parse_errors = parse.errors();
    let root = parse.tree().syntax().clone();
    let target_range = text_range(span);
    let selected = select_covering_node(&root, &(span.start()..span.end()))?;

    if contains_error_element(&selected) {
        return Err(AstError::UnparsableSpan {
            start: span.start(),
            end: span.end(),
        });
    }

    if !parse_errors.is_empty() {
        warn!(
            start = span.start(),
            end = span.end(),
            errors = parse_errors.len(),
            "lowered AST span from source with parser recovery errors"
        );
    }

    debug_assert!(selected.text_range().contains_range(target_range));
    Ok(NormalisedTree::new(lower_node(&selected), span))
}

fn select_covering_node(root: &SyntaxNode, target: &Range<u32>) -> AstResult<SyntaxNode> {
    let nodes = root.descendants().collect::<Vec<_>>();
    let ranges = nodes
        .iter()
        .map(|node| range_to_u32(node.text_range()))
        .collect::<Vec<_>>();

    select_smallest_covering(&ranges, target)
        .map(|index| nodes[index].clone())
        .ok_or_else(|| AstError::SpanOutOfBounds {
            start: target.start,
            end: target.end,
            len: usize::from(root.text_range().end()),
        })
}

fn lower_node(node: &SyntaxNode) -> NormalisedNode {
    let mut children = Vec::new();
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(child_node) => children.push(lower_node(&child_node)),
            NodeOrToken::Token(token) if !token.kind().is_trivia() => {
                children.push(lower_token(&token));
            }
            NodeOrToken::Token(_) => {}
        }
    }

    NormalisedNode::new(kind_id(node.kind()), None, children)
}

fn lower_token(token: &SyntaxToken) -> NormalisedNode {
    NormalisedNode::new(
        kind_id(token.kind()),
        Some(leaf_class(token.kind())),
        Vec::new(),
    )
}

fn leaf_class(kind: SyntaxKind) -> LeafClass {
    if is_identifier_like(kind) {
        LeafClass::Ident
    } else if kind.is_literal() {
        LeafClass::Literal
    } else {
        LeafClass::Other
    }
}

fn is_identifier_like(kind: SyntaxKind) -> bool {
    kind == SyntaxKind::LIFETIME_IDENT || kind.is_any_identifier()
}

fn kind_id(kind: SyntaxKind) -> KindId {
    KindId::new(u16::from(kind))
}

fn text_range(span: ByteSpan) -> TextRange {
    TextRange::new(TextSize::from(span.start()), TextSize::from(span.end()))
}

fn range_to_u32(range: TextRange) -> Range<u32> {
    u32::from(range.start())..u32::from(range.end())
}

fn contains_error_element(node: &SyntaxNode) -> bool {
    node.descendants_with_tokens()
        .any(|element| element.kind() == SyntaxKind::ERROR)
}

#[cfg(test)]
fn kind_name(kind: KindId) -> String {
    let parser_kind = SyntaxKind::from(kind.get());
    format!("{parser_kind:?}")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{kind_id, kind_name};
    use crate::{
        AstError, ByteSpan,
        ast::{KindId, LeafClass, NormalisedNode, NormalisedTree},
        lower_span,
    };

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
        let tree =
            lower_span_for("fn f() { call(value); }", "value").expect("identifier should lower");

        assert_ne!(kind_name(tree.root().kind()), "SOURCE_FILE");
        assert!(contains_leaf_class(tree.root(), LeafClass::Ident));
    }

    #[rstest]
    fn two_sibling_span_selects_common_expression_ancestor() {
        let tree =
            lower_span_for("fn f() { let value = a + b; }", "a + b").expect("span should lower");

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
    fn literal_tokens_lower_as_normalised_literal_leaves() {
        let tree =
            lower_span_for("fn f() { let value = 42; }", "42").expect("literal should lower");

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

    fn lower_span_for(source: &str, needle: &str) -> Result<NormalisedTree, AstError> {
        let start = source
            .find(needle)
            .ok_or(AstError::UnparsableSpan { start: 0, end: 0 })?;
        let end = start + needle.len();
        lower_span(source, ByteSpan::new(source, start as u32, end as u32)?)
    }

    fn contains_leaf_class(node: &NormalisedNode, leaf_class: LeafClass) -> bool {
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
        let node = NormalisedNode::new(kind_id(kind), Some(super::leaf_class(kind)), Vec::new());

        assert_eq!(node.leaf(), Some(leaf));
    }

    #[rstest]
    fn kind_names_are_available_for_adapter_snapshots() {
        assert_eq!(kind_name(KindId::new(0)), "TOMBSTONE");
    }
}
