//! Tests for the rustc hygiene to provenance adapter.

use rstest::rstest;
use rustc_span::{
    DUMMY_SP,
    Span,
    SyntaxContext,
    hygiene::{AstPass, DesugaringKind, ExpnKind, MacroKind},
    sym,
};
use whitaker_common::node_origin::{ExpansionMark, NodeOrigin};

use super::{MarkCursor, is_generated, node_origin};
use crate::hir::hygiene_test_support::{mark, test_span};

fn bang() -> ExpnKind { ExpnKind::Macro(MacroKind::Bang, sym::include) }

fn attribute() -> ExpnKind { ExpnKind::Macro(MacroKind::Attr, sym::include) }

fn derive() -> ExpnKind { ExpnKind::Macro(MacroKind::Derive, sym::include) }

fn ast_pass(pass: AstPass) -> ExpnKind { ExpnKind::AstPass(pass) }

fn desugaring(kind: DesugaringKind) -> ExpnKind { ExpnKind::Desugaring(kind) }

/// Builds a span whose marks are applied innermost first.
fn span_with_marks(applied: &[ExpnKind]) -> Span {
    let ctxt = applied.iter().fold(SyntaxContext::root(), |ctxt, kind| {
        mark(ctxt, kind.clone(), DUMMY_SP)
    });
    test_span(10, 20).with_ctxt(ctxt)
}

#[rstest]
#[case::root(vec![], NodeOrigin::Authored)]
#[case::bang_macro(vec![bang()], NodeOrigin::Generated)]
#[case::attribute_macro(vec![attribute()], NodeOrigin::Generated)]
#[case::derive_macro(vec![derive()], NodeOrigin::Generated)]
#[case::standard_imports(
    vec![ast_pass(AstPass::StdImports)],
    NodeOrigin::Generated
)]
#[case::test_harness(vec![ast_pass(AstPass::TestHarness)], NodeOrigin::Generated)]
#[case::proc_macro_harness(
    vec![ast_pass(AstPass::ProcMacroHarness)],
    NodeOrigin::Generated
)]
#[case::question_mark(
    vec![desugaring(DesugaringKind::QuestionMark)],
    NodeOrigin::Authored
)]
#[case::for_loop(
    vec![desugaring(DesugaringKind::ForLoop)],
    NodeOrigin::Authored
)]
#[case::desugaring_over_macro(
    vec![bang(), desugaring(DesugaringKind::ForLoop)],
    NodeOrigin::Generated
)]
#[case::desugaring_over_ast_pass(
    vec![ast_pass(AstPass::TestHarness), desugaring(DesugaringKind::Async)],
    NodeOrigin::Generated
)]
#[case::nested_macros(
    vec![bang(), bang()],
    NodeOrigin::Generated
)]
#[case::two_desugarings_over_macro(
    vec![
        bang(),
        desugaring(DesugaringKind::ForLoop),
        desugaring(DesugaringKind::Async),
    ],
    NodeOrigin::Generated
)]
#[case::macro_over_desugaring(
    vec![desugaring(DesugaringKind::ForLoop), bang()],
    NodeOrigin::Generated
)]
fn synthetic_contexts_classify(#[case] applied: Vec<ExpnKind>, #[case] expected: NodeOrigin) {
    rustc_span::create_default_session_globals_then(|| {
        let span = span_with_marks(&applied);

        assert_eq!(node_origin(span), expected);
        assert_eq!(is_generated(span), expected.is_generated());
    });
}

/// The cursor yields the outermost mark first, matching the policy contract.
#[test]
fn mark_cursor_yields_the_outermost_mark_first() {
    rustc_span::create_default_session_globals_then(|| {
        let span = span_with_marks(&[bang(), desugaring(DesugaringKind::ForLoop)]);
        let mut cursor = MarkCursor::new(span.ctxt());

        assert_eq!(cursor.next(), Some(ExpansionMark::Desugaring));
        assert_eq!(cursor.next(), Some(ExpansionMark::Macro));
        assert_eq!(cursor.next(), None);
    });
}

/// `SyntaxContext::marks` is innermost first, the opposite of the cursor.
///
/// The policy is order-insensitive today, but the adapter must not pass
/// `marks()` straight to `classify` if a future rule starts depending on
/// which mark is outermost.
#[test]
fn syntax_context_marks_are_innermost_first() {
    rustc_span::create_default_session_globals_then(|| {
        let span = span_with_marks(&[bang(), desugaring(DesugaringKind::ForLoop)]);
        let observed: Vec<_> = span
            .ctxt()
            .marks()
            .into_iter()
            .map(|(expn, _transparency)| expn.expn_data().kind)
            .collect();

        assert_eq!(observed, [bang(), desugaring(DesugaringKind::ForLoop)]);
    });
}
