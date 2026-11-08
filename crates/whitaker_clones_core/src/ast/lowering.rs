//! Adapter from parser syntax trees into the parser-agnostic AST domain.

use std::{cell::Cell, ops::Range, time::Instant};

use ra_ap_syntax::{
    AstNode,
    Edition,
    NodeOrToken,
    SourceFile,
    SyntaxKind,
    SyntaxNode,
    SyntaxToken,
    TextRange,
    TextSize,
};
use tracing::debug;

use super::{
    AstError,
    AstResult,
    ByteSpan,
    KindId,
    LeafClass,
    NormalizedNode,
    NormalizedTree,
    cover::{candidate_covers_target, should_select_covering_candidate},
    lowering_diagnostics::{
        log_covering_failure,
        log_lowering_depth_budget,
        log_lowering_failure,
        log_lowering_node_budget,
        log_parser_recovery,
        log_selection_depth_budget,
        log_selection_node_budget,
        log_span_validation_failure,
    },
    metrics::record_lower_span_metrics,
};
pub use crate::hashing::PARSER_SCHEMA_VERSION;

const MAX_AST_NODES: usize = 10_000;
const MAX_AST_DEPTH: usize = 256;

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
/// # Errors
///
/// Returns an [`AstError`] when the span fails re-validation against
/// `file_text`, when no syntax node covers the requested span, when the
/// selected subtree contains parser error elements, or when lowering exceeds
/// the depth or node budgets.
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

/// Lower the span and report whether parsing recovered.
fn lower_span_inner(file_text: &str, span: ByteSpan) -> (AstResult<NormalizedTree>, bool) {
    let parse = match validate_requested_span(file_text, span) {
        Ok(()) => SourceFile::parse(file_text, Edition::CURRENT),
        Err(error) => return (Err(error), false),
    };
    let recovered = report_parser_recovery(span, parse.errors().len());
    (lower_parsed_source(parse.tree().syntax(), span), recovered)
}

/// Re-validate `span` against `file_text` before any parsing work.
fn validate_requested_span(file_text: &str, span: ByteSpan) -> AstResult<()> {
    ByteSpan::new(file_text, span.start(), span.end())
        .map(|_| ())
        .map_err(log_span_validation_failure)
}

/// Report parser recovery errors and return whether the parse recovered.
fn report_parser_recovery(span: ByteSpan, error_count: usize) -> bool {
    if error_count == 0 {
        return false;
    }
    log_parser_recovery(span, error_count);
    true
}

/// Lower the selected covering subtree of an already parsed source file.
fn lower_parsed_source(root: &SyntaxNode, span: ByteSpan) -> AstResult<NormalizedTree> {
    let target_range = text_range(span);
    let selected = select_covering_node(root, span).map_err(log_covering_failure)?;
    log_selected_node(&selected);

    // Lowering doubles as parser-error detection: a single descent both builds
    // the normalized subtree and rejects any `ERROR` node or token, so the
    // selected span is never walked twice.
    let lowered = LoweringLimits::new(span)
        .lower(&selected, 0)
        .map_err(|error| log_lowering_failure(span, error))?;

    debug_assert!(
        selected.text_range().contains_range(target_range),
        "the selected covering node must contain the requested span"
    );
    Ok(NormalizedTree::new(lowered, span))
}

/// Report the syntax node selected to cover the requested span.
///
/// This is the one diagnostic that names a parser type. It lives here rather
/// than beside the other reports so that `lowering_diagnostics` stays free of
/// parser crates, as the AST boundary test requires.
fn log_selected_node(node: &SyntaxNode) {
    debug!(
        kind = ?node.kind(),
        span_width = u32::from(node.text_range().len()),
        "selected AST covering node"
    );
}

fn validate_covering_node_budget(span: ByteSpan, depth: usize, node_count: usize) -> AstResult<()> {
    if depth > MAX_AST_DEPTH {
        log_selection_depth_budget(span, depth, MAX_AST_DEPTH);
        return Err(AstError::TreeTooDeep {
            limit: MAX_AST_DEPTH,
        });
    }
    if node_count == MAX_AST_NODES {
        log_selection_node_budget(span, node_count, MAX_AST_NODES);
        return Err(AstError::TreeTooLarge {
            limit: MAX_AST_NODES,
        });
    }

    Ok(())
}

/// Check whether a syntax node's range fully contains the target range.
fn node_covers_target(node: &SyntaxNode, target: &Range<u32>) -> bool {
    candidate_covers_target(&range_to_u32(node.text_range()), target)
}

/// Records `node` as the current best when it satisfies the shared selection rule.
///
/// The parser-independent range predicate is also used by
/// [`super::select_smallest_covering`], which Kani verifies without parsing.
fn update_smallest_covering_node(
    selected: &mut Option<(SyntaxNode, u32)>,
    node: &SyntaxNode,
    target: &Range<u32>,
) {
    let range = range_to_u32(node.text_range());
    let width = range.end - range.start;
    let selected_width = selected.as_ref().map(|(_, selected_width)| *selected_width);

    if should_select_covering_candidate(&range, target, selected_width) {
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
        update_smallest_covering_node(&mut selected, &node, &target);

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
    /// Maximum normalized nodes, including non-trivia token leaves, permitted
    /// in the lowered subtree.
    maximum_nodes: usize,
    /// Requested candidate span, reported when the subtree is unparsable.
    span: ByteSpan,
    /// Normalized nodes emitted so far, including token leaves.
    lowered_node_count: Cell<usize>,
}

impl LoweringLimits {
    /// Create the default depth and node limits for a source span.
    const fn new(span: ByteSpan) -> Self { Self::with_depth_limit(MAX_AST_DEPTH, span) }

    /// Create limits with a selected depth bound and the standard node bound.
    const fn with_depth_limit(maximum_depth: usize, span: ByteSpan) -> Self {
        Self {
            maximum_depth,
            maximum_nodes: MAX_AST_NODES,
            span,
            lowered_node_count: Cell::new(0),
        }
    }

    /// Reserve one normalized node at `depth`, enforcing both lowering budgets.
    fn claim_node(&self, depth: usize) -> AstResult<()> {
        if depth > self.maximum_depth {
            log_lowering_depth_budget(self.span, depth, self.maximum_depth);
            return Err(AstError::TreeTooDeep {
                limit: self.maximum_depth,
            });
        }
        let lowered_nodes = self.lowered_node_count.get();
        if lowered_nodes == self.maximum_nodes {
            log_lowering_node_budget(self.span, lowered_nodes, self.maximum_nodes);
            return Err(AstError::TreeTooLarge {
                limit: self.maximum_nodes,
            });
        }
        self.lowered_node_count.set(lowered_nodes + 1);
        Ok(())
    }

    /// Lowers `node` while rejecting any parser `ERROR` node or token in the
    /// same descent, so error detection needs no separate subtree walk. The
    /// running normalized-node count bounds the lowered subtree, including
    /// token leaves, so an oversized selection cannot turn a parse into
    /// unbounded lowering work.
    fn lower(&self, node: &SyntaxNode, depth: usize) -> AstResult<NormalizedNode> {
        self.claim_node(depth)?;
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
                    self.claim_node(depth + 1)?;
                    children.push(lower_token(&token));
                }
                NodeOrToken::Token(_) => {}
            }
        }

        Ok(NormalizedNode::new(kind_id(node.kind()), None, children))
    }

    /// Build an error describing the span that could not be parsed.
    const fn unparsable_span(&self) -> AstError {
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

fn kind_id(kind: SyntaxKind) -> KindId { KindId::new(u16::from(kind)) }

fn text_range(span: ByteSpan) -> TextRange {
    TextRange::new(TextSize::from(span.start()), TextSize::from(span.end()))
}

fn range_to_u32(range: TextRange) -> Range<u32> { u32::from(range.start())..u32::from(range.end()) }

#[cfg(test)]
#[path = "lowering_metrics_tests.rs"]
mod metrics_tests;
#[cfg(test)]
#[path = "lowering_tests.rs"]
mod tests;
