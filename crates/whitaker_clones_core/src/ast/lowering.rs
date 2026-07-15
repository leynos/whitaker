//! Adapter from parser syntax trees into the parser-agnostic AST domain.

use std::ops::Range;

use ra_ap_syntax::{
    AstNode, Edition, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken, TextRange,
    TextSize,
};
use tracing::{debug, error, warn};

use super::{
    AstError, AstResult, ByteSpan, KindId, LeafClass, NormalisedNode, NormalisedTree,
    select_smallest_covering,
};

pub use crate::hashing::PARSER_SCHEMA_VERSION;

const MAX_EXPECTED_NODES: usize = 100_000;

fn trace_ast_error(
    error: AstError,
    span_out_of_bounds_message: &'static str,
    generic_message: &'static str,
) -> AstError {
    if let AstError::SpanOutOfBounds { start, end, len } = &error {
        error!(
            start = *start,
            end = *end,
            len = *len,
            "{span_out_of_bounds_message}"
        );
    } else {
        error!(?error, "{generic_message}");
    }
    error
}

/// Parses `file_text`, maps `span` to the smallest covering syntax node, and
/// lowers that subtree into a [`NormalisedTree`].
///
/// The [`ByteSpan`] is deliberately re-validated against `file_text` even
/// though callers pass an already constructed span. A `ByteSpan` proves only
/// that the offsets were valid for the text used to construct it; callers can
/// reuse a span across calls or accidentally pair it with different source
/// text. This defence-in-depth check is not redundant and must remain in place
/// even though it resembles double validation.
///
/// Latency metrics and feature-vector emission metrics are deferred to 7.3.2,
/// where scoring and SARIF emission consume those observations.
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
#[tracing::instrument(skip(file_text), fields(start = span.start(), end = span.end()))]
pub fn lower_span(file_text: &str, span: ByteSpan) -> AstResult<NormalisedTree> {
    let span = ByteSpan::new(file_text, span.start(), span.end()).map_err(|error| {
        trace_ast_error(
            error,
            "AST span lies outside the supplied source text",
            "AST span validation failed",
        )
    })?;
    let parse = SourceFile::parse(file_text, Edition::CURRENT);
    let parse_errors = parse.errors();
    if !parse_errors.is_empty() {
        // This is the designated logging boundary for parser recovery in this
        // adapter; the lowered AST domain remains parser-agnostic.
        warn!(
            start = span.start(),
            end = span.end(),
            errors = parse_errors.len(),
            "lowered AST span from source with parser recovery errors"
        );
    }
    let root = parse.tree().syntax().clone();
    let target_range = text_range(span);
    let selected = select_covering_node(&root, &(span.start()..span.end())).map_err(|error| {
        trace_ast_error(
            error,
            "no AST syntax node covers the requested span",
            "AST covering-node selection failed",
        )
    })?;
    debug!(
        kind = ?selected.kind(),
        span_width = u32::from(selected.text_range().len()),
        "selected AST covering node"
    );

    if contains_error_element(&selected) {
        error!(
            start = span.start(),
            end = span.end(),
            "selected AST span contains parser error elements"
        );
        return Err(AstError::UnparsableSpan {
            start: span.start(),
            end: span.end(),
        });
    }

    debug_assert!(selected.text_range().contains_range(target_range));
    Ok(NormalisedTree::new(lower_node(&selected), span))
}

/// Selects the smallest parser syntax node covering `target`.
///
/// `root.descendants().collect::<Vec<_>>()` is O(n) in the number of syntax
/// nodes in the parsed file, not just the candidate span. That cost is
/// acceptable here because `ra_ap_syntax::SourceFile::parse` bounds `root` to a
/// single source file (`min_nodes` upstream constraint), not an entire crate or
/// workspace. Callers must not pass multi-megabyte source files without
/// accepting this per-file traversal cost.
fn select_covering_node(root: &SyntaxNode, target: &Range<u32>) -> AstResult<SyntaxNode> {
    let nodes = root.descendants().collect::<Vec<_>>();
    debug_assert!(
        nodes.len() < MAX_EXPECTED_NODES,
        "unexpectedly large syntax tree ({} nodes); investigate candidate span sizing",
        nodes.len()
    );
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
#[path = "lowering_tests.rs"]
mod tests;
