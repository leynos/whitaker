//! Seeded-fault tests for the candidate-selection predicate.
//!
//! Each test constructs the faulty predicate beside the real one and asserts
//! that they disagree on a witness record. For the order-sensitive faults the
//! witness must fail both gates, otherwise the swapped order reports the same
//! reason and the test would pass against the fault. This file is separate from
//! the exhaustive enumeration so neither outgrows the file-size limit.

use super::{
    BodyPresence,
    CandidateFacts,
    Documentation,
    FunctionContext,
    HarnessStatus,
    PathSelection,
    Provenance,
    SkipReason,
    Verdict,
    select,
    selection_tests::{all_records, should_diagnose},
};

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
///
/// The witness must fail *both* gates for the order to be observable. A record
/// that fails only one reports the same reason under either order, so it would
/// pass against a swapped predicate and prove nothing. A whole-list scan asserts
/// that at least one such witness exists, so this test cannot silently degrade
/// into that vacuous form.
#[test]
fn a_predicate_with_swapped_path_and_context_checks_is_rejected() {
    let both_gates_fail = CandidateFacts {
        harness: HarnessStatus::TestHarness,
        provenance: Provenance::Source,
        context: FunctionContext::TraitImpl,
        body: BodyPresence::Present,
        path: PathSelection::NotSelected,
        documentation: Documentation::Undocumented,
    };
    assert_eq!(
        select(both_gates_fail),
        Verdict::Skip(SkipReason::OutOfScopeContext),
        "the real predicate must report the earlier context gate when both fail"
    );

    // The fault under test: context and path checked in the other order.
    let swapped = |facts: CandidateFacts| {
        if facts.harness == HarnessStatus::NotTestHarness {
            return Verdict::Skip(SkipReason::NotTestHarness);
        }
        if facts.provenance == Provenance::Expansion {
            return Verdict::Skip(SkipReason::NotSourceAuthored);
        }
        if facts.path == PathSelection::NotSelected {
            return Verdict::Skip(SkipReason::PathNotSelected);
        }
        if !matches!(
            facts.context,
            FunctionContext::Module | FunctionContext::InherentImpl | FunctionContext::Trait
        ) {
            return Verdict::Skip(SkipReason::OutOfScopeContext);
        }
        if facts.body == BodyPresence::Absent {
            return Verdict::Skip(SkipReason::Bodyless);
        }
        if facts.documentation == Documentation::Documented {
            return Verdict::Skip(SkipReason::Documented);
        }
        Verdict::Diagnose
    };
    assert_eq!(
        swapped(both_gates_fail),
        Verdict::Skip(SkipReason::PathNotSelected),
        "the seeded fault must report the path gate first, unlike the real predicate"
    );
    assert_ne!(
        select(both_gates_fail),
        swapped(both_gates_fail),
        "the order of the path and context gates must be observable"
    );

    // Non-vacuity: such a witness exists in the enumerated domain.
    assert!(
        all_records().into_iter().any(|facts| {
            facts.harness == HarnessStatus::TestHarness
                && facts.provenance == Provenance::Source
                && facts.path == PathSelection::NotSelected
                && !matches!(
                    facts.context,
                    FunctionContext::Module
                        | FunctionContext::InherentImpl
                        | FunctionContext::Trait
                )
        }),
        "the domain must contain a record that fails both the path and context gates"
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
