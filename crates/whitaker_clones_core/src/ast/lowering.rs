//! Adapter from parser syntax trees into the parser-agnostic AST domain.

use std::cell::Cell;
use std::ops::Range;
use std::time::Instant;

use ra_ap_syntax::{
    AstNode, Edition, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken, TextRange,
    TextSize,
};
use tracing::{debug, error, warn};

use super::metrics::record_lower_span_metrics;
use super::{AstError, AstResult, ByteSpan, KindId, LeafClass, NormalizedNode, NormalizedTree};

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
/// lowers that subtree into a [`NormalizedTree`].
///
/// The [`ByteSpan`] is deliberately re-validated against `file_text` even
/// though callers pass an already constructed span. A `ByteSpan` proves only
/// that the offsets were valid for the text used to construct it; callers can
/// reuse a span across calls or accidentally pair it with different source
/// text. This defence-in-depth check is not redundant and must remain in place
/// even though it resembles double validation.
///
/// Latency and categorized outcome metrics emit at this lowering boundary.
/// Feature-vector emission metrics remain reserved for the 7.3.2 scoring and
/// SARIF Run 1 consumption boundary.
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
pub fn lower_span(file_text: &str, span: ByteSpan) -> AstResult<NormalizedTree> {
    let started_at = Instant::now();
    let (result, recovered) = lower_span_inner(file_text, span);
    record_lower_span_metrics(&result, started_at.elapsed(), recovered);
    result
}

fn lower_span_inner(file_text: &str, span: ByteSpan) -> (AstResult<NormalizedTree>, bool) {
    let span = match ByteSpan::new(file_text, span.start(), span.end()).map_err(|error| {
        trace_ast_error(
            error,
            "AST span lies outside the supplied source text",
            "AST span validation failed",
        )
    }) {
        Ok(span) => span,
        Err(error) => return (Err(error), false),
    };
    let parse = SourceFile::parse(file_text, Edition::CURRENT);
    let parse_errors = parse.errors();
    let recovered = !parse_errors.is_empty();
    if recovered {
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
    let selected = match select_covering_node(&root, span).map_err(|error| {
        trace_ast_error(
            error,
            "no AST syntax node covers the requested span",
            "AST covering-node selection failed",
        )
    }) {
        Ok(selected) => selected,
        Err(error) => return (Err(error), recovered),
    };
    debug!(
        kind = ?selected.kind(),
        span_width = u32::from(selected.text_range().len()),
        "selected AST covering node"
    );

    // Lowering doubles as parser-error detection: a single descent both builds
    // the normalized subtree and rejects any `ERROR` node or token, so the
    // selected span is never walked twice.
    let lowered = match LoweringLimits::new(span)
        .lower(&selected, 0)
        .map_err(|error| {
            if matches!(error, AstError::UnparsableSpan { .. }) {
                error!(
                    start = span.start(),
                    end = span.end(),
                    "selected AST span contains parser error elements"
                );
            }
            error
        }) {
        Ok(lowered) => lowered,
        Err(error) => return (Err(error), recovered),
    };

    debug_assert!(selected.text_range().contains_range(target_range));
    (Ok(NormalizedTree::new(lowered, span)), recovered)
}

fn validate_covering_node_budget(span: ByteSpan, depth: usize, node_count: usize) -> AstResult<()> {
    if depth > MAX_AST_DEPTH {
        error!(
            start = span.start(),
            end = span.end(),
            depth,
            limit = MAX_AST_DEPTH,
            "AST covering-node selection exceeded the depth budget"
        );
        return Err(AstError::TreeTooDeep {
            limit: MAX_AST_DEPTH,
        });
    }
    if node_count == MAX_AST_NODES {
        error!(
            start = span.start(),
            end = span.end(),
            node_count,
            limit = MAX_AST_NODES,
            "AST covering-node selection exceeded the node budget"
        );
        return Err(AstError::TreeTooLarge {
            limit: MAX_AST_NODES,
        });
    }

    Ok(())
}

fn node_covers_target(node: &SyntaxNode, target: &Range<u32>) -> bool {
    let range = range_to_u32(node.text_range());
    range.start <= target.start && range.end >= target.end
}

/// Records `node` as the current best when it is narrower than the incumbent.
///
/// Callers must confirm `node` covers the target before calling this; the
/// smallest such node encountered wins, and the strict `<` comparison keeps the
/// first of any equal-width covering nodes (the shallowest, given pre-order
/// descent).
fn update_smallest_covering_node(selected: &mut Option<(SyntaxNode, u32)>, node: &SyntaxNode) {
    let range = range_to_u32(node.text_range());
    let width = range.end - range.start;
    let is_strictly_smaller = selected
        .as_ref()
        .is_none_or(|(_, selected_width)| width < *selected_width);

    if is_strictly_smaller {
        *selected = Some((node.clone(), width));
    }
}

/// Selects the smallest parser syntax node covering `target`.
///
/// Traversal is bounded to nodes on covering paths for the requested candidate
/// span rather than to the whole parsed file. Sibling syntax nodes hold
/// disjoint ranges, so only a child whose range fully covers `target` can be,
/// or contain, a smaller covering node; the walk descends into just those
/// children and skips unrelated subtrees without counting them toward the
/// budget. A narrow candidate therefore visits only its root-to-target
/// ancestry and is never rejected merely because the surrounding file is large.
/// A whole-file candidate can still require walking the full tree — but during
/// lowering of the selected root, where the node budget guards against oversized
/// input, not during selection.
///
/// The depth and node budgets bound the descent. Among covering nodes with the
/// same minimal width, the first (shallowest) encountered is retained.
fn select_covering_node(root: &SyntaxNode, span: ByteSpan) -> AstResult<SyntaxNode> {
    let target = span.start()..span.end();
    let mut selected = None;
    let mut node_count = 0_usize;
    let mut pending = vec![(root.clone(), 0_usize)];

    while let Some((node, depth)) = pending.pop() {
        validate_covering_node_budget(span, depth, node_count)?;
        node_count += 1;

        // A node that does not fully cover the target can neither be, nor
        // contain, the smallest covering node, so skip it and its subtree.
        if !node_covers_target(&node, &target) {
            continue;
        }
        update_smallest_covering_node(&mut selected, &node);

        pending.extend(
            node.children()
                .filter(|child| node_covers_target(child, &target))
                .map(|child| (child, depth + 1)),
        );
    }

    selected
        .map(|(node, _)| node)
        .ok_or_else(|| AstError::SpanOutOfBounds {
            start: target.start,
            end: target.end,
            len: usize::from(root.text_range().end()),
        })
}

/// Recursion-scoped invariants for lowering a covering subtree.
///
/// Bundling the depth and node budgets with the requested span keeps the
/// recursive [`lower`](LoweringLimits::lower) signature small while giving every
/// level the span it needs to report [`AstError::UnparsableSpan`] on parser
/// `ERROR` elements. The node budget lives here, not in covering-node
/// selection, so it bounds the subtree actually lowered rather than the whole
/// parsed file.
struct LoweringLimits {
    /// Maximum syntax depth permitted during lowering.
    maximum_depth: usize,
    /// Maximum syntax nodes permitted in the lowered subtree.
    maximum_nodes: usize,
    /// Requested candidate span, reported when the subtree is unparsable.
    span: ByteSpan,
    /// Nodes lowered so far, accumulated across the recursive descent.
    node_count: Cell<usize>,
}

impl LoweringLimits {
    fn new(span: ByteSpan) -> Self {
        Self::with_depth_limit(MAX_AST_DEPTH, span)
    }

    fn with_depth_limit(maximum_depth: usize, span: ByteSpan) -> Self {
        Self {
            maximum_depth,
            maximum_nodes: MAX_AST_NODES,
            span,
            node_count: Cell::new(0),
        }
    }

    /// Lowers `node` while rejecting any parser `ERROR` node or token in the
    /// same descent, so error detection needs no separate subtree walk. The
    /// running node count bounds the lowered subtree so an oversized selection
    /// cannot turn a single parse into unbounded lowering work.
    fn lower(&self, node: &SyntaxNode, depth: usize) -> AstResult<NormalizedNode> {
        if depth > self.maximum_depth {
            error!(
                start = self.span.start(),
                end = self.span.end(),
                depth,
                limit = self.maximum_depth,
                "AST lowering exceeded the depth budget"
            );
            return Err(AstError::TreeTooDeep {
                limit: self.maximum_depth,
            });
        }
        let lowered_nodes = self.node_count.get();
        if lowered_nodes == self.maximum_nodes {
            error!(
                start = self.span.start(),
                end = self.span.end(),
                node_count = lowered_nodes,
                limit = self.maximum_nodes,
                "AST lowering exceeded the node budget"
            );
            return Err(AstError::TreeTooLarge {
                limit: self.maximum_nodes,
            });
        }
        self.node_count.set(lowered_nodes + 1);
        if node.kind() == SyntaxKind::ERROR {
            return Err(self.unparsable_span());
        }

        let mut children = Vec::new();
        for child in node.children_with_tokens() {
            match child {
                NodeOrToken::Node(child_node) => {
                    children.push(self.lower(&child_node, depth + 1)?);
                }
                NodeOrToken::Token(token) if token.kind() == SyntaxKind::ERROR => {
                    return Err(self.unparsable_span());
                }
                NodeOrToken::Token(token) if !token.kind().is_trivia() => {
                    children.push(lower_token(&token));
                }
                NodeOrToken::Token(_) => {}
            }
        }

        Ok(NormalizedNode::new(kind_id(node.kind()), None, children))
    }

    fn unparsable_span(&self) -> AstError {
        AstError::UnparsableSpan {
            start: self.span.start(),
            end: self.span.end(),
        }
    }
}

fn lower_token(token: &SyntaxToken) -> NormalizedNode {
    NormalizedNode::new(
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

#[cfg(test)]
#[path = "lowering_tests.rs"]
mod tests;
