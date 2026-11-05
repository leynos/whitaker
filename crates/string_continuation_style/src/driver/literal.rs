//! Shared predicates identifying candidate cooked strings and their macro
//! positions.
//!
//! Two passes must agree on what counts as a candidate: the wrapper-depth
//! visitor records safe spans while walking expanded syntax, and the lint hook
//! re-checks a single expression before rewriting it. If those predicates
//! drifted apart, a span would be recorded for one shape and looked up for
//! another, so the test lives here and both callers share it.
//!
//! The expansion-frame test lives here too. It answers "which macro did the
//! user type?", which is the question the format-macro condition asks, and it
//! needs the same span the candidate test selects.

use rustc_ast::{
    ast::{self, Expr, ExprKind, FormatArgs},
    token::{Lit, LitKind},
};
use rustc_span::{ExpnKind, MacroKind, Span, Symbol};

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

/// Returns the raw body when `arguments` carries a source-authored cooked
/// format string containing a source-line join.
///
/// The three cheap facts this needs are all on `FormatArgs`, so the check
/// touches no source map: `is_source_literal` rejects a respanned
/// proc-macro literal, the uncooked symbol's kind rejects raw strings and the
/// `println!` family's generated newline, and the symbol scan rejects a body
/// with no join. The `println!` family appends its newline to the *cooked*
/// symbol only, so the raw body is exactly what the user wrote.
pub(crate) fn join_format_string(arguments: &FormatArgs) -> Option<&str> {
    // Bind by reference so the returned body borrows from `arguments` rather
    // than from a copy of the interned symbol.
    let (LitKind::Str, symbol) = &arguments.uncooked_fmt_str else {
        return None;
    };
    let body = symbol.as_str();
    let is_join = arguments.is_source_literal && body.contains("\\\n");
    is_join.then_some(body)
}

/// Reports whether any format argument is an implicit capture.
///
/// Captured width and precision count: `println!("{value:width$}")` captures
/// both, and neither still compiles once the format string is a `concat!()`.
pub(crate) fn has_captured_argument(arguments: &FormatArgs) -> bool {
    arguments
        .arguments
        .all_args()
        .iter()
        .any(|argument| matches!(argument.kind, ast::FormatArgumentKind::Captured(_)))
}

/// Returns the name of the outermost non-root expansion frame, if any.
///
/// The frame's name is the macro path the user wrote, so a `println!` facade
/// reports `println` and a fully qualified call reports its joined path.
/// Returning `None` for a source literal keeps the caller's gate honest: a
/// literal with no expansion ancestor is never in a macro position.
pub(crate) fn outermost_macro_name(span: Span) -> Option<Symbol> {
    let frame = span.source_callee()?;
    match frame.kind {
        ExpnKind::Macro(MacroKind::Bang, name) => Some(name),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../tests/support/literal_tests.rs"]
mod tests;
