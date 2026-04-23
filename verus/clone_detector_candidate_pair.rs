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

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExFragmentId(FragmentId);

enum CandidatePairOutcome<Id> {
    NoPair,
    Pair(Id, Id),
}

pub uninterp spec fn fragment_id_rank(id: &FragmentId) -> nat;

pub open spec fn fragment_id_partial_cmp_model(
    left: &FragmentId,
    right: &FragmentId,
) -> Option<Ordering> {
    if fragment_id_rank(left) < fragment_id_rank(right) {
        Some(Ordering::Less)
    } else if fragment_id_rank(left) > fragment_id_rank(right) {
        Some(Ordering::Greater)
    } else {
        Some(Ordering::Equal)
    }
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

pub assume_specification[ <FragmentId as PartialEq<FragmentId>>::eq ](
    left: &FragmentId,
    right: &FragmentId,
) -> bool;

pub assume_specification[ <FragmentId as PartialOrd<FragmentId>>::partial_cmp ](
    left: &FragmentId,
    right: &FragmentId,
) -> Option<Ordering>;

pub open spec fn strict_total_order_axioms<Id: PartialOrd>() -> bool {
    &&& forall|id: Id| #[trigger] id.eq_spec(&id)
    &&& forall|left: Id, right: Id| #[trigger] left.partial_cmp_spec(&right)
        != None::<Ordering>
    &&& forall|left: Id, right: Id| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Equal) <==> left.eq_spec(&right)
    &&& forall|left: Id, right: Id| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Less) <==> right.partial_cmp_spec(&left)
            == Some(Ordering::Greater)
    &&& forall|left: Id, right: Id| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Greater) <==> right.partial_cmp_spec(
            &left,
        ) == Some(Ordering::Less)
    &&& forall|left: Id, middle: Id, right: Id|
        left.partial_cmp_spec(&middle) == Some(Ordering::Less) && #[trigger]
            middle.partial_cmp_spec(&right) == Some(Ordering::Less) ==> #[trigger]
            left.partial_cmp_spec(&right) == Some(Ordering::Less)
}

proof fn lemma_fragment_id_partial_cmp_obeys_strict_total_order()
    ensures
        strict_total_order_axioms::<FragmentId>(),
{
    assert forall|id: FragmentId| #[trigger] id.eq_spec(&id) by {
        assert(fragment_id_rank(&id) == fragment_id_rank(&id));
    };

    assert forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) != None::<Ordering> by {
        if fragment_id_rank(&left) < fragment_id_rank(&right) {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Less));
        } else if fragment_id_rank(&left) > fragment_id_rank(&right) {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Greater));
        } else {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Equal));
        }
    };

    assert forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Equal) <==> left.eq_spec(&right) by {
        if fragment_id_rank(&left) < fragment_id_rank(&right) {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Less));
            assert(!left.eq_spec(&right));
        } else if fragment_id_rank(&left) > fragment_id_rank(&right) {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Greater));
            assert(!left.eq_spec(&right));
        } else {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Equal));
            assert(left.eq_spec(&right));
        }
    };

    assert forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Less) <==> right.partial_cmp_spec(&left)
            == Some(Ordering::Greater) by {
        if fragment_id_rank(&left) < fragment_id_rank(&right) {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Less));
            assert(right.partial_cmp_spec(&left) == Some(Ordering::Greater));
        } else if fragment_id_rank(&left) > fragment_id_rank(&right) {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Greater));
            assert(right.partial_cmp_spec(&left) == Some(Ordering::Less));
        } else {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Equal));
            assert(right.partial_cmp_spec(&left) == Some(Ordering::Equal));
        }
    };

    assert forall|left: FragmentId, right: FragmentId| #[trigger]
        left.partial_cmp_spec(&right) == Some(Ordering::Greater) <==> right.partial_cmp_spec(
            &left,
        ) == Some(Ordering::Less) by {
        if fragment_id_rank(&left) < fragment_id_rank(&right) {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Less));
            assert(right.partial_cmp_spec(&left) == Some(Ordering::Greater));
        } else if fragment_id_rank(&left) > fragment_id_rank(&right) {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Greater));
            assert(right.partial_cmp_spec(&left) == Some(Ordering::Less));
        } else {
            assert(left.partial_cmp_spec(&right) == Some(Ordering::Equal));
            assert(right.partial_cmp_spec(&left) == Some(Ordering::Equal));
        }
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

spec fn candidate_pair_new_result<Id: PartialOrd>(left: Id, right: Id) -> CandidatePairOutcome<Id> {
    if left.eq_spec(&right) {
        CandidatePairOutcome::NoPair
    } else if left.partial_cmp_spec(&right) == Some(Ordering::Less) {
        CandidatePairOutcome::Pair(left, right)
    } else {
        CandidatePairOutcome::Pair(right, left)
    }
}

spec fn constructor_accepts<Id: PartialOrd>(left: Id, right: Id) -> bool {
    match candidate_pair_new_result(left, right) {
        CandidatePairOutcome::Pair(_, _) => true,
        CandidatePairOutcome::NoPair => false,
    }
}

spec fn is_canonical_pair<Id: PartialOrd>(left: Id, right: Id, pair_left: Id, pair_right: Id) -> bool {
    pair_left.partial_cmp_spec(&pair_right) == Some(Ordering::Less)
        && ((pair_left.eq_spec(&left) && pair_right.eq_spec(&right))
            || (pair_left.eq_spec(&right) && pair_right.eq_spec(&left)))
}

proof fn lemma_equal_inputs_are_suppressed<Id: PartialOrd>(id: Id)
    requires
        strict_total_order_axioms::<Id>(),
    ensures
        candidate_pair_new_result(id, id) == CandidatePairOutcome::<Id>::NoPair,
        !constructor_accepts(id, id),
{
}

proof fn lemma_ordered_inputs_are_preserved<Id: PartialOrd>(left: Id, right: Id)
    requires
        strict_total_order_axioms::<Id>(),
        left.partial_cmp_spec(&right) == Some(Ordering::Less),
    ensures
        candidate_pair_new_result(left, right) == CandidatePairOutcome::Pair(left, right),
        constructor_accepts(left, right),
        is_canonical_pair(left, right, left, right),
{
}

proof fn lemma_reversed_inputs_are_swapped<Id: PartialOrd>(left: Id, right: Id)
    requires
        strict_total_order_axioms::<Id>(),
        left.partial_cmp_spec(&right) == Some(Ordering::Greater),
    ensures
        candidate_pair_new_result(left, right) == CandidatePairOutcome::Pair(right, left),
        constructor_accepts(left, right),
        is_canonical_pair(left, right, right, left),
{
    assert(right.partial_cmp_spec(&left) == Some(Ordering::Less));
}

proof fn lemma_distinct_inputs_yield_one_canonical_pair<Id: PartialOrd>(left: Id, right: Id)
    requires
        strict_total_order_axioms::<Id>(),
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
    match left.partial_cmp_spec(&right) {
        Some(Ordering::Less) => {
            lemma_ordered_inputs_are_preserved(left, right);
        }
        Some(Ordering::Equal) => {
            assert(left.eq_spec(&right));
            assert(false);
        }
        Some(Ordering::Greater) => {
            lemma_reversed_inputs_are_swapped(left, right);
        }
        None => {
            assert(false);
        }
    }
}

proof fn lemma_constructor_contract<Id: PartialOrd>(left: Id, right: Id)
    requires
        strict_total_order_axioms::<Id>(),
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
    if left.eq_spec(&right) {
        lemma_equal_inputs_are_suppressed(left);
    } else {
        lemma_distinct_inputs_yield_one_canonical_pair(left, right);
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
    lemma_constructor_contract(left, right);
}

fn main() {
}

} // verus!
