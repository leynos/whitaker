//! Verus proof for the `CandidatePair::new` constructor contract.
//!
//! This sidecar mirrors the runtime three-way branch structure exactly:
//! identical IDs are rejected first, already ordered distinct IDs are
//! preserved second, and reversed distinct IDs are swapped last.
//!
//! The proof models fragment identifiers as natural numbers standing in for an
//! ordered identifier domain. That keeps the proof focused on constructor
//! control flow instead of claiming to verify Rust `String` internals or the
//! compiled `FragmentId` implementation directly. Runtime unit and behaviour
//! tests pin the concrete lexical-string ordering contract.

use vstd::prelude::*;

verus! {

enum CandidatePairOutcome {
    NoPair,
    Pair(nat, nat),
}

spec fn candidate_pair_new_result(left: nat, right: nat) -> CandidatePairOutcome {
    if left == right {
        CandidatePairOutcome::NoPair
    } else if left < right {
        CandidatePairOutcome::Pair(left, right)
    } else {
        CandidatePairOutcome::Pair(right, left)
    }
}

spec fn constructor_accepts(left: nat, right: nat) -> bool {
    match candidate_pair_new_result(left, right) {
        CandidatePairOutcome::Pair(_, _) => true,
        CandidatePairOutcome::NoPair => false,
    }
}

spec fn is_canonical_pair(left: nat, right: nat, pair_left: nat, pair_right: nat) -> bool {
    pair_left < pair_right
        && ((pair_left == left && pair_right == right)
            || (pair_left == right && pair_right == left))
}

proof fn lemma_equal_inputs_are_suppressed(id: nat)
    ensures
        candidate_pair_new_result(id, id) == CandidatePairOutcome::NoPair,
        !constructor_accepts(id, id),
{
}

proof fn lemma_ordered_inputs_are_preserved(left: nat, right: nat)
    requires
        left < right,
    ensures
        candidate_pair_new_result(left, right) == CandidatePairOutcome::Pair(left, right),
        constructor_accepts(left, right),
        is_canonical_pair(left, right, left, right),
{
}

proof fn lemma_reversed_inputs_are_swapped(left: nat, right: nat)
    requires
        left > right,
    ensures
        candidate_pair_new_result(left, right) == CandidatePairOutcome::Pair(right, left),
        constructor_accepts(left, right),
        is_canonical_pair(left, right, right, left),
{
    assert(right < left);
}

proof fn lemma_distinct_inputs_yield_one_canonical_pair(left: nat, right: nat)
    requires
        left != right,
    ensures
        constructor_accepts(left, right),
        match candidate_pair_new_result(left, right) {
            CandidatePairOutcome::Pair(pair_left, pair_right) => {
                is_canonical_pair(left, right, pair_left, pair_right)
            }
            CandidatePairOutcome::NoPair => false,
        },
{
    if left < right {
        lemma_ordered_inputs_are_preserved(left, right);
    } else {
        lemma_reversed_inputs_are_swapped(left, right);
    }
}

proof fn lemma_constructor_contract(left: nat, right: nat)
    ensures
        !constructor_accepts(left, right) <==> left == right,
        constructor_accepts(left, right) <==> left != right,
        match candidate_pair_new_result(left, right) {
            CandidatePairOutcome::Pair(pair_left, pair_right) => {
                is_canonical_pair(left, right, pair_left, pair_right)
            }
            CandidatePairOutcome::NoPair => left == right,
        },
{
    if left == right {
        lemma_equal_inputs_are_suppressed(left);
    } else {
        lemma_distinct_inputs_yield_one_canonical_pair(left, right);
    }
}

proof fn lemma_documented_examples()
    ensures
        candidate_pair_new_result(1, 1) == CandidatePairOutcome::NoPair,
        candidate_pair_new_result(1, 2) == CandidatePairOutcome::Pair(1, 2),
        candidate_pair_new_result(2, 1) == CandidatePairOutcome::Pair(1, 2),
        candidate_pair_new_result(10, 2) == CandidatePairOutcome::Pair(2, 10),
{
    lemma_equal_inputs_are_suppressed(1);
    lemma_ordered_inputs_are_preserved(1, 2);
    lemma_reversed_inputs_are_swapped(2, 1);
    lemma_reversed_inputs_are_swapped(10, 2);
}

fn main() {
}

} // verus!
