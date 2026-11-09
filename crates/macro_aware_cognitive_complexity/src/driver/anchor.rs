//! Resolves the diagnostic anchor, subject, and wrapper classification.
//!
//! An item or method anchors on its identifier, matching pinned Clippy. A
//! closure anchors on its declaration header, because the synthetic coroutine
//! wrappers rustc creates for `async` constructs have no closure syntax in
//! source and must be told apart from authored closures.

use log::debug;
use rustc_hir as hir;
use rustc_hir::{ClosureKind, def_id::LocalDefId, intravisit::FnKind};
use rustc_lint::LateContext;
use rustc_span::{Ident, Span};

use super::LINT_NAME;

/// Subject reported for a closure diagnostic.
pub(super) const CLOSURE_SUBJECT: &str = "<closure>";

/// The span a diagnostic points at together with its reported subject.
pub(super) struct FunctionAnchor {
    /// The span the diagnostic underlines.
    pub(super) span: Span,
    /// The identifier text, or [`CLOSURE_SUBJECT`], named by the message.
    pub(super) subject: String,
}

/// Resolve the anchor and subject for one function-like item.
///
/// Fall back to the function span when a closure's HIR node cannot be read.
/// That should not happen for a `check_fn` call, so the fallback logs at debug
/// level rather than failing the lint pass.
pub(super) fn resolve(
    cx: &LateContext<'_>,
    kind: FnKind<'_>,
    span: Span,
    def_id: LocalDefId,
) -> FunctionAnchor {
    match kind {
        FnKind::ItemFn(ident, ..) | FnKind::Method(ident, ..) => identified(ident),
        FnKind::Closure => closure_anchor(cx, span, def_id),
    }
}

/// Report whether the function-like item is an `async fn`.
///
/// Closures are never `async fn` items even when their body was desugared from
/// an async block, so they report `false` here.
pub(super) fn is_async(kind: FnKind<'_>) -> bool {
    match kind {
        FnKind::ItemFn(_, _, header) => header.asyncness.is_async(),
        FnKind::Method(_, signature) => signature.header.asyncness.is_async(),
        FnKind::Closure => false,
    }
}

/// Report whether a closure is a compiler-synthesized coroutine wrapper.
///
/// rustc lowers every `async fn` and `async` block into a closure whose kind
/// is `Coroutine`. Their spans still point at authored source, so the span
/// guards cannot reject them; this classification is what keeps one source
/// construct from producing two diagnostics.
pub(super) fn is_coroutine_wrapper(
    cx: &LateContext<'_>,
    kind: FnKind<'_>,
    def_id: LocalDefId,
) -> bool {
    if !matches!(kind, FnKind::Closure) {
        return false;
    }

    let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);
    match cx.tcx.hir_node(hir_id) {
        hir::Node::Expr(expr) => matches!(
            expr.kind,
            hir::ExprKind::Closure(closure) if matches!(closure.kind, ClosureKind::Coroutine(_))
        ),
        _ => false,
    }
}

/// Build an anchor from a declared item or method identifier.
fn identified(ident: Ident) -> FunctionAnchor {
    FunctionAnchor {
        span: ident.span,
        subject: ident.name.as_str().to_owned(),
    }
}

/// Build a closure anchor from its declaration header.
fn closure_anchor(cx: &LateContext<'_>, fallback: Span, def_id: LocalDefId) -> FunctionAnchor {
    let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);

    if let hir::Node::Expr(expr) = cx.tcx.hir_node(hir_id)
        && let hir::ExprKind::Closure(closure) = expr.kind
    {
        return FunctionAnchor {
            span: closure.fn_decl_span,
            subject: String::from(CLOSURE_SUBJECT),
        };
    }

    debug!(
        target: LINT_NAME,
        "closure HIR node unavailable for {def_id:?}; anchoring on the function span"
    );
    FunctionAnchor {
        span: fallback,
        subject: String::from(CLOSURE_SUBJECT),
    }
}
