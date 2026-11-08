//! Classifies HIR spans as authored source or generated code.
//!
//! rustc records macro and AST-pass expansions as marks on a syntax context.
//! The pure policy in [`whitaker_common::node_origin`] decides which marks make
//! a node generated. This adapter walks the context one mark at a time so it
//! can feed that policy without allocating the whole chain.

use rustc_span::{
    Span,
    SyntaxContext,
    hygiene::{ExpnId, ExpnKind},
};
use whitaker_common::node_origin::{ExpansionMark, NodeOrigin, classify};

#[cfg(test)]
mod tests;

/// Classifies the HIR node at `span` as authored or generated.
///
/// A root context is authored immediately. Desugaring marks are transparent,
/// so a `for` loop or `?` written in source remains authored even though rustc
/// lowered it. Any macro or AST-pass mark makes the node generated.
///
/// # Examples
///
/// ```ignore
/// # use rustc_span::DUMMY_SP;
/// # use whitaker::node_origin;
/// # use whitaker_common::node_origin::NodeOrigin;
/// assert_eq!(node_origin(DUMMY_SP), NodeOrigin::Authored);
/// ```
#[must_use]
pub fn node_origin(span: Span) -> NodeOrigin { classify(MarkCursor::new(span.ctxt())) }

/// Reports whether the HIR node at `span` was generated.
///
/// # Examples
///
/// ```ignore
/// # use rustc_span::DUMMY_SP;
/// # use whitaker::is_generated;
/// assert!(!is_generated(DUMMY_SP));
/// ```
#[must_use]
pub fn is_generated(span: Span) -> bool { node_origin(span).is_generated() }

/// Iterates a syntax context's expansion marks, outermost first.
pub(super) struct MarkCursor {
    ctxt: SyntaxContext,
}

impl MarkCursor {
    /// Starts at `ctxt` and walks towards the root.
    pub(super) const fn new(ctxt: SyntaxContext) -> Self { Self { ctxt } }
}

impl Iterator for MarkCursor {
    type Item = ExpansionMark;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ctxt.is_root() {
            return None;
        }

        expansion_mark(self.ctxt.remove_mark())
    }
}

/// Maps a rustc expansion to the policy's three-way mark.
///
/// The `Expr` and `Name` carried by macro marks do not affect provenance, so
/// every macro kind collapses to [`ExpansionMark::Macro`]. `ExpnKind` is
/// matched exhaustively rather than with a wildcard, so a new rustc variant
/// fails compilation instead of being silently misclassified.
fn expansion_mark(expn: ExpnId) -> Option<ExpansionMark> {
    match expn.expn_data().kind {
        ExpnKind::Root => None,
        ExpnKind::Desugaring(_) => Some(ExpansionMark::Desugaring),
        ExpnKind::Macro(..) => Some(ExpansionMark::Macro),
        ExpnKind::AstPass(_) => Some(ExpansionMark::AstPass),
    }
}
