//! Verus proofs for decomposition vector algebra helpers.
//!
//! This module models Whitaker's sparse feature vectors as aligned sequences of
//! non-negative weights. It proves the algebraic properties used by the
//! decomposition advice implementation: dot-product commutativity, squared-norm
//! non-negativity, and zero dot product when no feature index has positive
//! weight in both vectors.

use vstd::prelude::*;

verus! {

spec fn dot_product(left: Seq<nat>, right: Seq<nat>) -> nat
    recommends
        left.len() == right.len(),
    decreases left.len()
{
    if left.len() == 0 {
        0nat
    } else {
        left[0] * right[0]
            + dot_product(
                left.subrange(1, left.len() as int),
                right.subrange(1, right.len() as int),
            )
    }
}

spec fn norm_squared(vector: Seq<nat>) -> nat {
    dot_product(vector, vector)
}

spec fn no_overlapping_positive_features(left: Seq<nat>, right: Seq<nat>) -> bool
    recommends
        left.len() == right.len(),
{
    forall|index: int|
        0 <= index < left.len() ==> !(left[index] > 0 && right[index] > 0)
}

proof fn lemma_no_overlap_is_preserved_for_tail(left: Seq<nat>, right: Seq<nat>)
    requires
        left.len() == right.len(),
        0 < left.len(),
        no_overlapping_positive_features(left, right),
    ensures
        no_overlapping_positive_features(
            left.subrange(1, left.len() as int),
            right.subrange(1, right.len() as int),
        ),
{
}

#[verifier::nonlinear]
proof fn lemma_dot_product_commutative(left: Seq<nat>, right: Seq<nat>)
    requires
        left.len() == right.len(),
    ensures
        dot_product(left, right) == dot_product(right, left),
    decreases left.len()
{
    if left.len() > 0 {
        lemma_dot_product_commutative(
            left.subrange(1, left.len() as int),
            right.subrange(1, right.len() as int),
        );
    }
}

proof fn lemma_norm_squared_is_non_negative(vector: Seq<nat>)
    ensures
        0 <= norm_squared(vector),
{
}

#[verifier::nonlinear]
proof fn lemma_zero_dot_product_without_overlapping_positive_features(
    left: Seq<nat>,
    right: Seq<nat>,
)
    requires
        left.len() == right.len(),
        no_overlapping_positive_features(left, right),
    ensures
        dot_product(left, right) == 0,
    decreases left.len()
{
    if left.len() > 0 {
        lemma_no_overlap_is_preserved_for_tail(left, right);
        lemma_zero_dot_product_without_overlapping_positive_features(
            left.subrange(1, left.len() as int),
            right.subrange(1, right.len() as int),
        );
        assert(!(left[0] > 0 && right[0] > 0));
        assert(left[0] * right[0] == 0);
    }
}

proof fn lemma_exact_overlap_has_positive_norm_and_commutative_dot()
    ensures
        dot_product(seq![6nat, 2nat], seq![6nat, 0nat]) == 36nat,
        dot_product(seq![6nat, 2nat], seq![6nat, 0nat])
            == dot_product(seq![6nat, 0nat], seq![6nat, 2nat]),
        norm_squared(seq![6nat, 2nat]) == 40nat,
{
    lemma_dot_product_commutative(seq![6nat, 2nat], seq![6nat, 0nat]);
    assert(dot_product(seq![6nat, 2nat], seq![6nat, 0nat]) == 36nat) by (compute);
    assert(norm_squared(seq![6nat, 2nat]) == 40nat) by (compute);
}

proof fn lemma_disjoint_positive_features_have_zero_dot_product()
    ensures
        no_overlapping_positive_features(seq![6nat, 0nat, 2nat], seq![0nat, 5nat, 0nat]),
        dot_product(seq![6nat, 0nat, 2nat], seq![0nat, 5nat, 0nat]) == 0,
{
    lemma_zero_dot_product_without_overlapping_positive_features(
        seq![6nat, 0nat, 2nat],
        seq![0nat, 5nat, 0nat],
    );
    assert(dot_product(seq![6nat, 0nat, 2nat], seq![0nat, 5nat, 0nat]) == 0) by (compute);
}

fn main() {
}

} // verus!
