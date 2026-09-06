//! Tests for HIR span recovery helpers and their macro-expansion behaviour.
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
//!   (`example_compiles_under_test_harness`)

use super::{recover_user_editable_hir_span, span_recovery_frames};
use rstest::{fixture, rstest};
use rustc_data_structures::stable_hash::{
    RawDefId, RawDefPathHash, RawSpan, StableHashControls, StableHashCtxt, StableHasher,
};
use rustc_span::edition::Edition;
use rustc_span::hygiene::{ExpnData, ExpnKind, LocalExpnId, MacroKind, Transparency};
use rustc_span::{BytePos, DUMMY_SP, Span, SyntaxContext, sym};
use whitaker_common::SpanRecoveryFrame;

fn test_span(lo: u32, hi: u32) -> Span {
    Span::with_root_ctxt(BytePos(lo), BytePos(hi))
}

#[derive(Clone, Copy)]
struct TestHashStableContext;

impl StableHashCtxt for TestHashStableContext {
    fn stable_hash_span(&mut self, _span: RawSpan, _hasher: &mut StableHasher) {}

    fn def_path_hash(&self, _def_id: RawDefId) -> RawDefPathHash {
        RawDefPathHash([0; 16])
    }

    fn stable_hash_controls(&self) -> StableHashControls {
        StableHashControls { hash_spans: false }
    }

    fn assert_default_stable_hash_controls(&self, _msg: &str) {}
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
    MacroOnly,
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
        SpanRecoveryCase::MacroOnly => {
            let expanded = expanded_span(test_span(30, 40), DUMMY_SP);
            (expanded, vec![SpanRecoveryFrame::new(expanded, true)], None)
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
#[case::macro_only(SpanRecoveryCase::MacroOnly)]
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

#[test]
fn macro_only_hir_span_has_no_user_editable_recovery() {
    rustc_span::create_default_session_globals_then(|| {
        let expanded = expanded_span(test_span(30, 40), DUMMY_SP);

        assert_eq!(recover_user_editable_hir_span(expanded), None);
    });
}

/// Tests for the `cfg(test)` recognition shared by the expect and unwrap lints.
///
/// These build `CfgEntry` values directly rather than through a compilation.
/// The recognizer is a pure function over that tree, and the shapes that
/// matter, negation and nesting, are precisely the ones a UI fixture cannot
/// express without a separate crate per case.
mod cfg_trace {
    use super::super::cfg_entry_gates_on_test;
    use rustc_data_structures::thin_vec::thin_vec;
    use rustc_hir::attrs::CfgEntry;
    use rustc_span::{DUMMY_SP, Symbol, create_default_session_globals_then};

    fn name(value: &str) -> CfgEntry {
        CfgEntry::NameValue {
            name: Symbol::intern(value),
            value: None,
            span: DUMMY_SP,
        }
    }

    #[test]
    fn a_bare_test_predicate_gates_on_test() {
        create_default_session_globals_then(|| {
            assert!(cfg_entry_gates_on_test(&name("test"), true));
        });
    }

    #[test]
    fn another_predicate_does_not() {
        create_default_session_globals_then(|| {
            assert!(!cfg_entry_gates_on_test(&name("unix"), true));
        });
    }

    #[test]
    fn negation_inverts_the_answer() {
        // `not(test)` leaves the item present only outside a test build, so
        // reading it as test context would exempt production code.
        create_default_session_globals_then(|| {
            let negated = CfgEntry::Not(Box::new(name("test")), DUMMY_SP);
            assert!(!cfg_entry_gates_on_test(&negated, true));
        });
    }

    #[test]
    fn double_negation_restores_it() {
        create_default_session_globals_then(|| {
            let inner = CfgEntry::Not(Box::new(name("test")), DUMMY_SP);
            let outer = CfgEntry::Not(Box::new(inner), DUMMY_SP);
            assert!(cfg_entry_gates_on_test(&outer, true));
        });
    }

    #[test]
    fn any_and_all_both_count() {
        // `all(test, unix)` is test-only, and `any(test, feature = "x")`
        // includes test builds. Either way the item is present when testing,
        // which is the question the caller is asking.
        create_default_session_globals_then(|| {
            for entry in [
                CfgEntry::All(thin_vec![name("test"), name("unix")], DUMMY_SP),
                CfgEntry::Any(thin_vec![name("test"), name("feature")], DUMMY_SP),
            ] {
                assert!(cfg_entry_gates_on_test(&entry, true));
            }
        });
    }

    #[test]
    fn a_group_without_test_does_not_count() {
        create_default_session_globals_then(|| {
            let entry = CfgEntry::All(thin_vec![name("unix"), name("feature")], DUMMY_SP);
            assert!(!cfg_entry_gates_on_test(&entry, true));
        });
    }

    #[test]
    fn negation_carries_into_a_group() {
        // `not(any(test, unix))` excludes test builds, so nothing inside it is
        // test context.
        create_default_session_globals_then(|| {
            let group = CfgEntry::Any(thin_vec![name("test"), name("unix")], DUMMY_SP);
            let negated = CfgEntry::Not(Box::new(group), DUMMY_SP);
            assert!(!cfg_entry_gates_on_test(&negated, true));
        });
    }

    #[test]
    fn literals_and_versions_are_not_test_gates() {
        create_default_session_globals_then(|| {
            assert!(!cfg_entry_gates_on_test(
                &CfgEntry::Bool(true, DUMMY_SP),
                true
            ));
            assert!(!cfg_entry_gates_on_test(
                &CfgEntry::Version(None, DUMMY_SP),
                true
            ));
        });
    }
}
