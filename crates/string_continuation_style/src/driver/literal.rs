//! Shared predicate identifying candidate cooked string literals.
//!
//! Two passes must agree on what counts as a candidate: the wrapper-depth
//! visitor records safe spans while walking expanded syntax, and the lint hook
//! re-checks a single expression before rewriting it. If those predicates
//! drifted apart, a span would be recorded for one shape and looked up for
//! another, so the test lives here and both callers share it.

use rustc_ast::{
    ast::{Expr, ExprKind},
    token::{Lit, LitKind},
};

/// Returns the literal when `expression` is a plain cooked string literal
/// containing a source-line join.
///
/// "Plain" means a `Str` literal with no suffix, which excludes raw strings,
/// byte strings, C strings, and suffixed forms that `concat!()` would not
/// preserve. The body test is deliberately syntactic: a literal without a
/// backslash-newline pair has nothing to rewrite, so it is rejected before any
/// expensive source recovery.
///
/// Returning the literal rather than a bare `bool` lets the rewrite path bind
/// its symbol without matching `ExprKind` a second time, where the unreachable
/// arm would be a silent invitation to weaken one copy and not the other.
pub(crate) fn plain_join_literal(expression: &Expr) -> Option<&Lit> {
    let ExprKind::Lit(literal) = &expression.kind else {
        return None;
    };
    let is_join = literal.kind == LitKind::Str
        && literal.suffix.is_none()
        && literal.symbol.as_str().contains("\\\n");
    is_join.then_some(literal)
}
