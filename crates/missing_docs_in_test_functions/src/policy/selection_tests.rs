//! Exhaustive tests for the candidate-selection predicate.
//!
//! The domain is finite — six fields with 2 × 2 × 5 × 2 × 2 × 2 = 160 records —
//! so enumerating the production predicate over every record is a complete
//! proof for every admissible input, with no model to drift. The oracle here is
//! written independently of the implementation: a conjunction over the six
//! facts, plus an ordered scan for the first failing gate, rather than the
//! implementation's chain of guard clauses.

use insta::assert_snapshot;
use proptest::prelude::*;
use rstest::rstest;

use super::{
    BodyPresence,
    CandidateFacts,
    ContainerKind,
    ContainerStack,
    DocAttributeShape,
    Documentation,
    FunctionContext,
    FunctionShape,
    HarnessStatus,
    PathSelection,
    Provenance,
    SkipReason,
    Verdict,
    documentation_status,
    select,
};

/// Every `HarnessStatus` value, in truth-table order.
const HARNESS: [HarnessStatus; 2] = [HarnessStatus::TestHarness, HarnessStatus::NotTestHarness];

/// Every `Provenance` value, in truth-table order.
const PROVENANCE: [Provenance; 2] = [Provenance::Source, Provenance::Expansion];

/// Every `FunctionContext` value, in truth-table order.
const CONTEXT: [FunctionContext; 5] = [
    FunctionContext::Module,
    FunctionContext::InherentImpl,
    FunctionContext::Trait,
    FunctionContext::TraitImpl,
    FunctionContext::Nested,
];

/// Every `BodyPresence` value, in truth-table order.
const BODY: [BodyPresence; 2] = [BodyPresence::Present, BodyPresence::Absent];

/// Every `PathSelection` value, in truth-table order.
const PATH: [PathSelection; 2] = [PathSelection::Selected, PathSelection::NotSelected];

/// Every `Documentation` value, in truth-table order.
const DOCUMENTATION: [Documentation; 2] = [Documentation::Undocumented, Documentation::Documented];

/// Enumerates every fact record exactly once.
fn all_records() -> Vec<CandidateFacts> {
    let mut records = Vec::with_capacity(160);
    for harness in HARNESS {
        for provenance in PROVENANCE {
            for context in CONTEXT {
                for body in BODY {
                    for path in PATH {
                        for documentation in DOCUMENTATION {
                            records.push(CandidateFacts {
                                harness,
                                provenance,
                                context,
                                body,
                                path,
                                documentation,
                            });
                        }
                    }
                }
            }
        }
    }
    records
}

/// The specification: diagnose exactly the in-scope, selected, undocumented
/// source functions in a test-harness compilation.
fn should_diagnose(facts: CandidateFacts) -> bool {
    facts.harness == HarnessStatus::TestHarness
        && facts.provenance == Provenance::Source
        && matches!(
            facts.context,
            FunctionContext::Module | FunctionContext::InherentImpl | FunctionContext::Trait
        )
        && facts.body == BodyPresence::Present
        && facts.path == PathSelection::Selected
        && facts.documentation == Documentation::Undocumented
}

/// The earliest gate that fails for `facts`, in contract order.
fn first_failing_gate(facts: CandidateFacts) -> Option<SkipReason> {
    let gates = [
        (
            facts.harness == HarnessStatus::NotTestHarness,
            SkipReason::NotTestHarness,
        ),
        (
            facts.provenance == Provenance::Expansion,
            SkipReason::NotSourceAuthored,
        ),
        (
            !matches!(
                facts.context,
                FunctionContext::Module | FunctionContext::InherentImpl | FunctionContext::Trait
            ),
            SkipReason::OutOfScopeContext,
        ),
        (facts.body == BodyPresence::Absent, SkipReason::Bodyless),
        (
            facts.path == PathSelection::NotSelected,
            SkipReason::PathNotSelected,
        ),
        (
            facts.documentation == Documentation::Documented,
            SkipReason::Documented,
        ),
    ];
    gates
        .into_iter()
        .find_map(|(is_failing, reason)| is_failing.then_some(reason))
}

/// Reports whether `g` is `f` with any subset of fields moved towards
/// suppression.
///
/// A field moves towards suppression when it becomes `NotTestHarness`,
/// `Expansion`, `TraitImpl` or `Nested`, `Absent`, `NotSelected`, or
/// `Documented`. For the two five-way context fields the relation is not a
/// lattice, so only the four suppressing context values count as moves away
/// from an in-scope context.
fn is_at_least_as_suppressed(facts: CandidateFacts, other: CandidateFacts) -> bool {
    let harness_ok =
        other.harness == facts.harness || other.harness == HarnessStatus::NotTestHarness;
    let provenance_ok =
        other.provenance == facts.provenance || other.provenance == Provenance::Expansion;
    let context_ok = other.context == facts.context
        || matches!(
            other.context,
            FunctionContext::TraitImpl | FunctionContext::Nested
        );
    let body_ok = other.body == facts.body || other.body == BodyPresence::Absent;
    let path_ok = other.path == facts.path || other.path == PathSelection::NotSelected;
    let documentation_ok = other.documentation == facts.documentation
        || other.documentation == Documentation::Documented;
    harness_ok && provenance_ok && context_ok && body_ok && path_ok && documentation_ok
}

/// The enumerator yields exactly 160 distinct records.
#[test]
fn enumeration_covers_every_record_once() {
    let records = all_records();
    assert_eq!(records.len(), 160, "the truth table must hold 160 records");
    let distinct: std::collections::HashSet<_> = records.iter().collect();
    assert_eq!(
        distinct.len(),
        records.len(),
        "every enumerated record must be distinct"
    );
}

/// At least one record diagnoses, so a predicate that never fires fails.
#[test]
fn at_least_one_record_diagnoses() {
    assert!(
        all_records().into_iter().any(should_diagnose),
        "the truth table must contain at least one diagnosable record"
    );
}

/// Every gate is the first failing gate for at least one record.
#[test]
fn every_gate_can_be_the_first_failure() {
    let records = all_records();
    for (reason, label) in [
        (SkipReason::NotTestHarness, "harness"),
        (SkipReason::NotSourceAuthored, "provenance"),
        (SkipReason::OutOfScopeContext, "context"),
        (SkipReason::Bodyless, "body"),
        (SkipReason::PathNotSelected, "path"),
        (SkipReason::Documented, "documentation"),
    ] {
        assert!(
            records
                .iter()
                .any(|facts| first_failing_gate(*facts) == Some(reason)),
            "the {label} gate must be the first failure for some record"
        );
    }
}

/// `L-SOUND`, `L-COMPLETE`, and `L-PRECEDENCE` hold for every record.
#[test]
fn selection_matches_the_specification_for_every_record() {
    let mut diagnosing = 0_usize;
    for facts in all_records() {
        let verdict = select(facts);
        if should_diagnose(facts) {
            diagnosing += 1;
            assert_eq!(
                verdict,
                Verdict::Diagnose,
                "record {facts:?} satisfies the specification but was skipped",
            );
        } else {
            assert_eq!(
                verdict,
                Verdict::Skip(
                    first_failing_gate(facts)
                        .expect("a record outside the specification has a failing gate")
                ),
                "record {facts:?} must be skipped for its first failing gate",
            );
        }
    }
    assert_eq!(
        diagnosing, 3,
        "the specification admits 1 x 1 x 3 x 1 x 1 x 1 records"
    );
}

/// `L-MONOTONE`: no change towards suppression can create a diagnostic.
#[test]
fn suppression_never_creates_a_diagnostic() {
    let records = all_records();
    for facts in &records {
        for other in &records {
            if !is_at_least_as_suppressed(*facts, *other) {
                continue;
            }
            assert!(
                select(*other) != Verdict::Diagnose || select(*facts) == Verdict::Diagnose,
                "moving {facts:?} towards suppression to {other:?} created a diagnostic",
            );
        }
    }
}

/// The full truth table, as stable text, one row per record.
#[test]
fn truth_table_snapshot_is_stable() {
    let mut table = String::new();
    for facts in all_records() {
        let verdict = match select(facts) {
            Verdict::Diagnose => String::from("Diagnose"),
            Verdict::Skip(reason) => format!("Skip({reason:?})"),
        };
        table.push_str(&format!(
            "{:<14} {:<10} {:<13} {:<8} {:<12} {:<13} => {verdict}\n",
            format!("{:?}", facts.harness),
            format!("{:?}", facts.provenance),
            format!("{:?}", facts.context),
            format!("{:?}", facts.body),
            format!("{:?}", facts.path),
            format!("{:?}", facts.documentation),
        ));
    }
    assert_snapshot!("truth_table", table);
}

/// A seeded fault: deleting the documentation gate must be rejected.
#[test]
fn a_predicate_without_the_documentation_gate_is_rejected() {
    let documented = CandidateFacts {
        harness: HarnessStatus::TestHarness,
        provenance: Provenance::Source,
        context: FunctionContext::Module,
        body: BodyPresence::Present,
        path: PathSelection::Selected,
        documentation: Documentation::Documented,
    };
    let faulty = |facts: CandidateFacts| {
        let mut reduced = facts;
        reduced.documentation = Documentation::Undocumented;
        select(reduced)
    };
    assert_eq!(
        select(documented),
        Verdict::Skip(SkipReason::Documented),
        "the real predicate must skip a documented function"
    );
    assert_eq!(
        faulty(documented),
        Verdict::Diagnose,
        "the seeded fault must diagnose what the real predicate skips"
    );
}

/// A seeded fault: swapping the path and context checks must be rejected.
#[test]
fn a_predicate_with_swapped_path_and_context_checks_is_rejected() {
    let out_of_scope_selected = CandidateFacts {
        harness: HarnessStatus::TestHarness,
        provenance: Provenance::Source,
        context: FunctionContext::TraitImpl,
        body: BodyPresence::Present,
        path: PathSelection::Selected,
        documentation: Documentation::Undocumented,
    };
    assert_eq!(
        select(out_of_scope_selected),
        Verdict::Skip(SkipReason::OutOfScopeContext),
        "the real predicate must report the earlier context gate"
    );
    assert_eq!(
        first_failing_gate(out_of_scope_selected),
        Some(SkipReason::OutOfScopeContext),
        "the oracle must agree on the gate order"
    );
}

/// A seeded fault: treating `TraitImpl` as in scope must be rejected.
#[test]
fn a_predicate_treating_trait_impl_as_in_scope_is_rejected() {
    let in_trait_impl = CandidateFacts {
        harness: HarnessStatus::TestHarness,
        provenance: Provenance::Source,
        context: FunctionContext::TraitImpl,
        body: BodyPresence::Present,
        path: PathSelection::Selected,
        documentation: Documentation::Undocumented,
    };
    assert_eq!(
        select(in_trait_impl),
        Verdict::Skip(SkipReason::OutOfScopeContext),
        "trait-implementation methods are out of scope"
    );
    assert!(
        !should_diagnose(in_trait_impl),
        "the oracle must exclude trait-implementation methods"
    );
}

/// The oracle is non-vacuous: dropping the context clause changes its answer.
#[test]
fn the_oracle_without_the_context_clause_disagrees() {
    let nested = CandidateFacts {
        harness: HarnessStatus::TestHarness,
        provenance: Provenance::Source,
        context: FunctionContext::Nested,
        body: BodyPresence::Present,
        path: PathSelection::Selected,
        documentation: Documentation::Undocumented,
    };
    let weakened = |facts: CandidateFacts| {
        facts.harness == HarnessStatus::TestHarness
            && facts.provenance == Provenance::Source
            && facts.body == BodyPresence::Present
            && facts.path == PathSelection::Selected
            && facts.documentation == Documentation::Undocumented
    };
    assert!(
        !should_diagnose(nested),
        "the oracle must exclude nested functions"
    );
    assert!(
        weakened(nested),
        "dropping the context clause must change the oracle's answer"
    );
}

/// Documentation shapes classify as their documented contract states.
#[rstest]
#[case(DocAttributeShape::DocComment, true)]
#[case(DocAttributeShape::DocNameValue, true)]
#[case(DocAttributeShape::CfgAttrDoc, true)]
#[case(DocAttributeShape::DocList, false)]
#[case(DocAttributeShape::Other, false)]
fn documentation_shapes_classify(#[case] shape: DocAttributeShape, #[case] is_docs: bool) {
    assert_eq!(
        documentation_status([shape]),
        if is_docs {
            Documentation::Documented
        } else {
            Documentation::Undocumented
        },
        "shape {shape:?} must classify as documentation == {is_docs}",
    );
}

proptest! {
    /// For any set of attribute shapes, the function is documented exactly when
    /// at least one shape counts as documentation.
    #[test]
    fn documentation_status_is_an_existential_over_shapes(
        shapes in prop::collection::vec(any_shape(), 0..=8_usize)
    ) {
        let expected = shapes.iter().any(|shape| shape.counts_as_documentation());
        prop_assert_eq!(
            documentation_status(shapes.clone()) == Documentation::Documented,
            expected,
        );
    }

    /// A free function is in scope exactly when every frame is a module.
    #[test]
    fn free_function_context_depends_only_on_module_frames(
        kinds in prop::collection::vec(any_container_kind(), 0..=6_usize)
    ) {
        let mut stack = ContainerStack::default();
        for kind in &kinds {
            stack.enter(*kind);
        }
        let expected = if kinds.iter().all(|kind| *kind == ContainerKind::Module) {
            FunctionContext::Module
        } else {
            FunctionContext::Nested
        };
        prop_assert_eq!(stack.classify(FunctionShape::Free), expected);
    }
}

/// Generates one `DocAttributeShape` value.
fn any_shape() -> impl Strategy<Value = DocAttributeShape> {
    prop_oneof![
        Just(DocAttributeShape::DocComment),
        Just(DocAttributeShape::DocNameValue),
        Just(DocAttributeShape::CfgAttrDoc),
        Just(DocAttributeShape::DocList),
        Just(DocAttributeShape::Other),
    ]
}

/// Generates one `ContainerKind` value.
fn any_container_kind() -> impl Strategy<Value = ContainerKind> {
    prop_oneof![
        Just(ContainerKind::Module),
        Just(ContainerKind::InherentImpl),
        Just(ContainerKind::TraitImpl),
        Just(ContainerKind::Trait),
        Just(ContainerKind::Other),
    ]
}
