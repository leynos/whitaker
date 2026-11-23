//! Driver-private unit tests for the diagnostic span choice.
//!
//! This is the one mapping in the driver whose two branches are reachable from
//! a hand-built value and whose failure would be silent: a diagnostic attached
//! to a dummy span is dropped from the report rather than misreported. Every
//! other driver mapping reads a shape only a real compilation produces, so the
//! UI fixtures cover those instead; a unit test that assembles an AST node by
//! hand would prove the match arm and not that rustc hands it that shape.

use rustc_span::{BytePos, DUMMY_SP, Span, SyntaxContext};

use super::diagnostics::diagnostic_span;

/// A non-dummy span, used where the distinction from [`DUMMY_SP`] matters.
fn real_span() -> Span { Span::new(BytePos(10), BytePos(20), SyntaxContext::root(), None) }

/// The item-span fallback fires only for a dummy identifier span.
#[test]
fn diagnostic_span_prefers_the_identifier_unless_it_is_dummy() {
    let item_span = real_span();
    assert_eq!(
        diagnostic_span(DUMMY_SP, item_span),
        item_span,
        "a dummy identifier span must fall back to the item span"
    );
    assert_eq!(
        diagnostic_span(item_span, DUMMY_SP),
        item_span,
        "a real identifier span must be used even when the item span is dummy"
    );
}
