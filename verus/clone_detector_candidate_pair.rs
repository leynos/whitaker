//! Verus proof for the `CandidatePair::new` constructor contract.
//!
//! This sidecar still mirrors the runtime three-way branch structure exactly:
//! identical IDs are rejected first, already ordered distinct IDs are
//! preserved second, and reversed distinct IDs are swapped last.
//!
//! The difference from the initial proof is that the ordered-ID model is now
//! made explicit as a trusted bridge over the real `FragmentId` source
//! definition. The proof does not verify Rust `String` internals or prove that
//! the derived `FragmentId::partial_cmp` implementation is lexicographic from
//! first principles. Instead, it introduces a ghost `nat` ranking for
//! `FragmentId`, uses that ranking to state the strict-total-order axioms
//! required by the constructor proof, and makes the trust boundary explicit in
//! one bridge lemma rather than leaving it only in prose.

use core::cmp::Ordering;

use vstd::prelude::*;
use vstd::std_specs::cmp::{PartialEqSpec, PartialOrdSpec};

#[path = "../crates/whitaker_clones_core/src/index/fragment_id.rs"]
mod fragment_id_runtime;

use fragment_id_runtime::FragmentId;

verus! {

/// Verus external type witness for the production `FragmentId` newtype.
#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExFragmentId(FragmentId);

enum CandidatePairOutcome {
    NoPair,
    Pair(FragmentId, FragmentId),
}

enum CandidatePairInputRelation {
    Same,
    Ordered,
    Reversed,
}

/// Trusted ghost rank used to bridge production `FragmentId` ordering.
///
/// Equal ranks are exactly `eq_spec`, and rank comparison is the trusted model
/// for `FragmentId::partial_cmp` on both sides of the proof.
pub uninterp spec fn fragment_id_rank(id: &FragmentId) -> nat;

/// Classifies two fragment IDs by their trusted ghost ranks.
pub open spec fn fragment_id_rank_relation(left: &FragmentId, right: &FragmentId) -> Ordering {
    if fragment_id_rank(left) < fragment_id_rank(right) {
        Ordering::Less
    } else if fragment_id_rank(left) > fragment_id_rank(right) {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// Models `FragmentId::partial_cmp` as a total comparison over ghost ranks.
pub open spec fn fragment_id_partial_cmp_model(
    left: &FragmentId,
    right: &FragmentId,
) -> Option<Ordering> {
    Some(fragment_id_rank_relation(left, right))
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for FragmentId {
    open spec fn obeys_eq_spec() -> bool {
        true
    }

    open spec fn eq_spec(&self, other: &FragmentId) -> bool {
        fragment_id_rank(self) == fragment_id_rank(other)
    }
}

impl vstd::std_specs::cmp::PartialOrdSpecImpl for FragmentId {
    open spec fn obeys_partial_cmp_spec() -> bool {
        true
    }

    open spec fn partial_cmp_spec(&self, other: &FragmentId) -> Option<Ordering> {
        fragment_id_partial_cmp_model(self, other)
    }
}

/// Trusts the production `PartialEq<FragmentId>` implementation for
/// `FragmentId::eq` to match its `eq_spec` counterpart.
///
/// This declaration is a trust boundary: the proof assumes the runtime
/// equality implementation and the Verus equality model describe the same
/// relation.
///
/// Runtime unit tests and BDD scenarios ground this assumption in concrete
/// lexical-string behaviour.
pub assume_specification[ <FragmentId as PartialEq<FragmentId>>::eq ](
    left: &FragmentId,
    right: &FragmentId,
) -> bool;

/// Trusts the production `PartialOrd<FragmentId>` implementation for
/// `FragmentId::partial_cmp` to match its `partial_cmp_spec` counterpart.
///
/// This declaration is a trust boundary: the proof assumes the runtime
/// ordering implementation and the Verus ordering model describe the same
/// relation.
///
/// Runtime unit tests and BDD scenarios ground this assumption in concrete
/// lexical-string behaviour.
pub assume_specification[ <FragmentId as PartialOrd<FragmentId>>::partial_cmp ](
    left: &FragmentId,
    right: &FragmentId,
) -> Option<Ordering>;

/// States the strict-total-order properties required by `CandidatePair::new`.
pub open spec fn fragment_id_strict_total_order_axioms() -> bool {
    &&& forall|id: FragmentId| #[trigger] id.eq_spec(&id)
    &&& forall|left: FragmentId, right: FragmentId| #[trigger] left.partial_cmp_spec(&right)
        != None::<Ordering>
    &&& forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Equal) <==> left.eq_spec(&right)
    &&& forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Less) <==> right.partial_cmp_spec(&left)
            == Some(Ordering::Greater)
    &&& forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Greater) <==> right.partial_cmp_spec(
            &left,
        ) == Some(Ordering::Less)
    &&& forall|left: FragmentId, middle: FragmentId, right: FragmentId|
        left.partial_cmp_spec(&middle) == Some(Ordering::Less) && #[trigger]
            middle.partial_cmp_spec(&right) == Some(Ordering::Less) ==> #[trigger]
            left.partial_cmp_spec(&right) == Some(Ordering::Less)
}

proof fn lemma_fragment_id_partial_cmp_matches_rank(left: FragmentId, right: FragmentId)
    ensures
        left.partial_cmp_spec(&right) == Some(fragment_id_rank_relation(&left, &right)),
        right.partial_cmp_spec(&left) == Some(fragment_id_rank_relation(&right, &left)),
{
}

proof fn lemma_fragment_id_partial_cmp_obeys_strict_total_order()
    ensures
        fragment_id_strict_total_order_axioms(),
{
    assert forall|id: FragmentId| #[trigger] id.eq_spec(&id) by {
        assert(fragment_id_rank(&id) == fragment_id_rank(&id));
    };

    assert forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) != None::<Ordering> by {
        lemma_fragment_id_partial_cmp_matches_rank(left, right);
    };

    assert forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Equal) <==> left.eq_spec(&right) by {
        lemma_fragment_id_partial_cmp_matches_rank(left, right);
    };

    assert forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Less) <==> right.partial_cmp_spec(&left)
            == Some(Ordering::Greater) by {
        lemma_fragment_id_partial_cmp_matches_rank(left, right);
    };

    assert forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Greater) <==> right.partial_cmp_spec(
            &left,
        ) == Some(Ordering::Less) by {
        lemma_fragment_id_partial_cmp_matches_rank(left, right);
    };

    assert forall|left: FragmentId, middle: FragmentId, right: FragmentId|
        left.partial_cmp_spec(&middle) == Some(Ordering::Less) && #[trigger]
            middle.partial_cmp_spec(&right) == Some(Ordering::Less) implies #[trigger]
            left.partial_cmp_spec(&right) == Some(Ordering::Less) by {
        if left.partial_cmp_spec(&middle) == Some(Ordering::Less)
            && middle.partial_cmp_spec(&right) == Some(Ordering::Less)
        {
            assert(fragment_id_rank(&left) < fragment_id_rank(&middle));
            assert(fragment_id_rank(&middle) < fragment_id_rank(&right));
            assert(fragment_id_rank(&left) < fragment_id_rank(&right));
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Less));
        }
    };
}

spec fn candidate_pair_input_relation(
    left: FragmentId,
    right: FragmentId,
) -> CandidatePairInputRelation {
    if left.eq_spec(&right) {
        CandidatePairInputRelation::Same
    } else if left.partial_cmp_spec(&right) == Some(Ordering::Less) {
        CandidatePairInputRelation::Ordered
    } else {
        CandidatePairInputRelation::Reversed
    }
}

spec fn candidate_pair_new_result(left: FragmentId, right: FragmentId) -> CandidatePairOutcome {
    match candidate_pair_input_relation(left, right) {
        CandidatePairInputRelation::Same => CandidatePairOutcome::NoPair,
        CandidatePairInputRelation::Ordered => CandidatePairOutcome::Pair(left, right),
        CandidatePairInputRelation::Reversed => CandidatePairOutcome::Pair(right, left),
    }
}

spec fn constructor_accepts(left: FragmentId, right: FragmentId) -> bool {
    match candidate_pair_new_result(left, right) {
        CandidatePairOutcome::Pair(_, _) => true,
        CandidatePairOutcome::NoPair => false,
    }
}

spec fn is_canonical_pair(
    left: FragmentId,
    right: FragmentId,
    pair_left: FragmentId,
    pair_right: FragmentId,
) -> bool {
    pair_left.partial_cmp_spec(&pair_right) == Some(Ordering::Less)
        && ((pair_left.eq_spec(&left) && pair_right.eq_spec(&right))
            || (pair_left.eq_spec(&right) && pair_right.eq_spec(&left)))
}

proof fn lemma_equal_inputs_are_suppressed(id: FragmentId)
    requires
        fragment_id_strict_total_order_axioms(),
    ensures
        candidate_pair_new_result(id, id) == CandidatePairOutcome::NoPair,
        !constructor_accepts(id, id),
{
}

proof fn lemma_ordered_inputs_are_preserved(left: FragmentId, right: FragmentId)
    requires
        fragment_id_strict_total_order_axioms(),
        left.partial_cmp_spec(&right) == Some(Ordering::Less),
    ensures
        candidate_pair_new_result(left, right) == CandidatePairOutcome::Pair(left, right),
        constructor_accepts(left, right),
        is_canonical_pair(left, right, left, right),
{
}

proof fn lemma_reversed_inputs_are_swapped(left: FragmentId, right: FragmentId)
    requires
        fragment_id_strict_total_order_axioms(),
        left.partial_cmp_spec(&right) == Some(Ordering::Greater),
    ensures
        candidate_pair_new_result(left, right) == CandidatePairOutcome::Pair(right, left),
        constructor_accepts(left, right),
        is_canonical_pair(left, right, right, left),
{
    assert(right.partial_cmp_spec(&left) == Some(Ordering::Less));
}

proof fn lemma_distinct_ordered_inputs_yield_canonical_pair(left: FragmentId, right: FragmentId)
    requires
        !left.eq_spec(&right),
        left.partial_cmp_spec(&right) == Some(Ordering::Less),
    ensures
        constructor_accepts(left, right),
        match candidate_pair_new_result(left, right) {
            CandidatePairOutcome::Pair(pair_left, pair_right) => {
                is_canonical_pair(left, right, pair_left, pair_right)
            }
            CandidatePairOutcome::NoPair => false,
        },
{
    lemma_ordered_inputs_are_preserved(left, right);
}

proof fn lemma_distinct_reversed_inputs_yield_canonical_pair(left: FragmentId, right: FragmentId)
    requires
        !left.eq_spec(&right),
        left.partial_cmp_spec(&right) == Some(Ordering::Greater),
    ensures
        constructor_accepts(left, right),
        match candidate_pair_new_result(left, right) {
            CandidatePairOutcome::Pair(pair_left, pair_right) => {
                is_canonical_pair(left, right, pair_left, pair_right)
            }
            CandidatePairOutcome::NoPair => false,
        },
{
    lemma_reversed_inputs_are_swapped(left, right);
}

proof fn lemma_distinct_inputs_yield_one_canonical_pair(left: FragmentId, right: FragmentId)
    requires
        fragment_id_strict_total_order_axioms(),
        !left.eq_spec(&right),
    ensures
        constructor_accepts(left, right),
        match candidate_pair_new_result(left, right) {
            CandidatePairOutcome::Pair(pair_left, pair_right) => {
                is_canonical_pair(left, right, pair_left, pair_right)
            }
            CandidatePairOutcome::NoPair => false,
        },
{
    if left.partial_cmp_spec(&right) == Some(Ordering::Less) {
        lemma_distinct_ordered_inputs_yield_canonical_pair(left, right);
    } else {
        assert(left.partial_cmp_spec(&right) == Some(Ordering::Greater));
        lemma_distinct_reversed_inputs_yield_canonical_pair(left, right);
    }
}

proof fn lemma_fragment_id_constructor_contract(left: FragmentId, right: FragmentId)
    ensures
        !constructor_accepts(left, right) <==> left.eq_spec(&right),
        constructor_accepts(left, right) <==> !left.eq_spec(&right),
        match candidate_pair_new_result(left, right) {
            CandidatePairOutcome::Pair(pair_left, pair_right) => {
                is_canonical_pair(left, right, pair_left, pair_right)
            }
            CandidatePairOutcome::NoPair => left.eq_spec(&right),
        },
{
    lemma_fragment_id_partial_cmp_obeys_strict_total_order();
    if left.eq_spec(&right) {
        lemma_equal_inputs_are_suppressed(left);
    } else {
        lemma_distinct_inputs_yield_one_canonical_pair(left, right);
    }
}

fn main() {
}

} // verus!
