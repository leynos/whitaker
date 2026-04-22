//! Tests for HIR span recovery helpers and their macro-expansion behavior.
//!
//! `collect_rstest_companion_test_functions` depends on a real
//! `rustc_lint::LateContext`, which is only available while rustc is walking
//! fully lowered HIR for an actual compilation session. The helper inspects
//! sibling HIR items, parent-module relationships, and harness-generated
//! companion modules, so there is no stable, lightweight unit-test seam that
//! can construct the required `LateContext` and HIR in isolation inside this
//! crate.
//!
//! Coverage therefore lives in the no-expect lint's UI/example harness
//! regressions, which exercise this detection path end-to-end with real rstest
//! expansion output:
//! - `crates/no_expect_outside_tests/examples/pass_expect_in_rstest_harness.rs`
//! - `crates/no_expect_outside_tests/src/lib_ui_tests.rs`
//!   (`rstest_example_compiles_under_test_harness`)

use super::{recover_user_editable_hir_span, span_recovery_frames};
use rstest::{fixture, rstest};
use rustc_data_structures::stable_hasher::HashingControls;
use rustc_span::def_id::{DefId, DefPathHash, LocalDefId};
use rustc_span::edition::Edition;
use rustc_span::hygiene::{ExpnData, ExpnKind, LocalExpnId, MacroKind, Transparency};
use rustc_span::{BytePos, DUMMY_SP, Span, SpanData, StableSourceFileId, SyntaxContext, sym};
use whitaker_common::SpanRecoveryFrame;

fn test_span(lo: u32, hi: u32) -> Span {
    Span::with_root_ctxt(BytePos(lo), BytePos(hi))
}

#[derive(Clone, Copy)]
struct TestHashStableContext;

impl rustc_span::HashStableContext for TestHashStableContext {
    fn def_path_hash(&self, _def_id: DefId) -> DefPathHash {
        DefPathHash::default()
    }

    fn hash_spans(&self) -> bool {
        false
    }

    fn unstable_opts_incremental_ignore_spans(&self) -> bool {
        true
    }

    fn def_span(&self, _def_id: LocalDefId) -> Span {
        DUMMY_SP
    }

    fn span_data_to_lines_and_cols(
        &mut self,
        _span: &SpanData,
    ) -> Option<(StableSourceFileId, usize, BytePos, usize, BytePos)> {
        None
    }

    fn hashing_controls(&self) -> HashingControls {
        HashingControls { hash_spans: false }
    }
}

fn expanded_span(span: Span, call_site: Span) -> Span {
    let expn_id = LocalExpnId::fresh_empty();
    expn_id.set_expn_data(
        ExpnData::default(
            ExpnKind::Macro(MacroKind::Bang, sym::include),
            call_site,
            Edition::Edition2024,
            None,
            None,
        ),
        TestHashStableContext,
    );

    span.with_ctxt(
        SyntaxContext::root().apply_mark(expn_id.to_expn_id(), Transparency::Transparent),
    )
}

#[derive(Clone, Copy)]
enum SpanRecoveryCase {
    Dummy,
    Direct,
    Recovered,
}

#[fixture]
fn build_span_case()
-> impl Fn(SpanRecoveryCase) -> (Span, Vec<SpanRecoveryFrame<Span>>, Option<Span>) {
    move |case| match case {
        SpanRecoveryCase::Dummy => (DUMMY_SP, vec![], None),
        SpanRecoveryCase::Direct => {
            let span = test_span(10, 20);
            (span, vec![SpanRecoveryFrame::new(span, false)], Some(span))
        }
        SpanRecoveryCase::Recovered => {
            let recovered = test_span(10, 20);
            let expanded = expanded_span(test_span(30, 40), recovered);

            (
                expanded,
                vec![
                    SpanRecoveryFrame::new(expanded, true),
                    SpanRecoveryFrame::new(recovered, false),
                ],
                Some(recovered),
            )
        }
    }
}

#[rstest]
#[case::dummy(SpanRecoveryCase::Dummy)]
#[case::direct(SpanRecoveryCase::Direct)]
#[case::recovered(SpanRecoveryCase::Recovered)]
fn span_recovery_walks_expected_frames(
    #[case] case: SpanRecoveryCase,
    build_span_case: impl Fn(SpanRecoveryCase) -> (Span, Vec<SpanRecoveryFrame<Span>>, Option<Span>),
) {
    rustc_span::create_default_session_globals_then(|| {
        let (span, expected_frames, expected_recovered_span) = build_span_case(case);

        assert_eq!(span_recovery_frames(span), expected_frames);
        assert_eq!(
            recover_user_editable_hir_span(span),
            expected_recovered_span
        );
    });
}
