//! Adapter from parser syntax trees into the parser-agnostic AST domain.

use std::ops::Range;

use ra_ap_syntax::{
    AstNode, Edition, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken, TextRange,
    TextSize,
};
use tracing::{debug, error, warn};

use super::{AstError, AstResult, ByteSpan, KindId, LeafClass, NormalisedNode, NormalisedTree};

pub use crate::hashing::PARSER_SCHEMA_VERSION;

const MAX_AST_NODES: usize = 10_000;
const MAX_AST_DEPTH: usize = 256;

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

    let lowered = lower_node(&selected, 0)?;
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
    Ok(NormalisedTree::new(lowered, span))
}

/// Selects the smallest parser syntax node covering `target`.
///
/// Traversal is O(n) in the parsed source file, not just the candidate span.
/// It enforces bounded node and depth budgets in every build so callers cannot
/// turn a single-file parse into unbounded lowering work.
fn select_covering_node(root: &SyntaxNode, target: &Range<u32>) -> AstResult<SyntaxNode> {
    let mut pending = vec![(root.clone(), 0_usize)];
    let mut selected = None;
    let mut node_count = 0_usize;

    while let Some((node, depth)) = pending.pop() {
        if depth > MAX_AST_DEPTH {
            return Err(AstError::TreeTooDeep {
                limit: MAX_AST_DEPTH,
            });
        }
        if node_count == MAX_AST_NODES {
            return Err(AstError::TreeTooLarge {
                limit: MAX_AST_NODES,
            });
        }
        node_count += 1;

        let range = range_to_u32(node.text_range());
        if range.start <= target.start && range.end >= target.end {
            let width = range.end - range.start;
            if selected
                .as_ref()
                .is_none_or(|(_, selected_width)| width < *selected_width)
            {
                selected = Some((node.clone(), width));
            }
        }

        let children = node.children().collect::<Vec<_>>();
        pending.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
    }

    selected
        .map(|(node, _)| node)
        .ok_or_else(|| AstError::SpanOutOfBounds {
            start: target.start,
            end: target.end,
            len: usize::from(root.text_range().end()),
        })
}

fn lower_node(node: &SyntaxNode, depth: usize) -> AstResult<NormalisedNode> {
    lower_node_with_limit(node, depth, MAX_AST_DEPTH)
}

fn lower_node_with_limit(
    node: &SyntaxNode,
    depth: usize,
    maximum_depth: usize,
) -> AstResult<NormalisedNode> {
    if depth > maximum_depth {
        return Err(AstError::TreeTooDeep {
            limit: maximum_depth,
        });
    }

    let mut children = Vec::new();
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(child_node) => children.push(lower_node_with_limit(
                &child_node,
                depth + 1,
                maximum_depth,
            )?),
            NodeOrToken::Token(token) if !token.kind().is_trivia() => {
                children.push(lower_token(&token));
            }
            NodeOrToken::Token(_) => {}
        }
    }

    Ok(NormalisedNode::new(kind_id(node.kind()), None, children))
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
