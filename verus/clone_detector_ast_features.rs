//! Verus proof for AST kind-count fold invariance.
//!
//! This sidecar models the exact `(kind, depth)` contributions produced by the
//! runtime AST walk. It proves the algebraic count update used by
//! `KindCounts`: adjacent contributions commute for every queried
//! `(kind, depth)`, and any two sequences with the same contribution multiset
//! fold to the same exact count for that queried pair. Production traversal
//! fidelity is covered by Rust tests and proptest over `NormalisedTree`; this
//! proof covers the pure accumulator algebra, not parser lowering or tree
//! traversal.

use vstd::prelude::*;

verus! {

pub struct Contribution {
    pub kind: nat,
    pub depth: nat,
}

pub open spec fn contribution_matches(contribution: Contribution, kind: nat, depth: nat) -> bool {
    contribution.kind == kind && contribution.depth == depth
}

pub open spec fn increment_count(
    accumulator: nat,
    contribution: Contribution,
    kind: nat,
    depth: nat,
) -> nat {
    if contribution_matches(contribution, kind, depth) {
        accumulator + 1nat
    } else {
        accumulator
    }
}

pub open spec fn fold_count_from(
    contributions: Seq<Contribution>,
    kind: nat,
    depth: nat,
    accumulator: nat,
) -> nat
    decreases contributions.len()
{
    if contributions.len() == 0 {
        accumulator
    } else {
        fold_count_from(
            contributions.drop_first(),
            kind,
            depth,
            increment_count(accumulator, contributions[0], kind, depth),
        )
    }
}

pub open spec fn fold_count(contributions: Seq<Contribution>, kind: nat, depth: nat) -> nat {
    fold_count_from(contributions, kind, depth, 0nat)
}

pub open spec fn matching_count(contributions: Seq<Contribution>, kind: nat, depth: nat) -> nat
    decreases contributions.len()
{
    if contributions.len() == 0 {
        0nat
    } else if contribution_matches(contributions[0], kind, depth) {
        1nat + matching_count(contributions.drop_first(), kind, depth)
    } else {
        matching_count(contributions.drop_first(), kind, depth)
    }
}

pub open spec fn same_contribution_multiset(
    left: Seq<Contribution>,
    right: Seq<Contribution>,
) -> bool {
    forall|kind: nat, depth: nat|
        matching_count(left, kind, depth) == matching_count(right, kind, depth)
}

proof fn lemma_increment_count_commutes(
    accumulator: nat,
    left: Contribution,
    right: Contribution,
    kind: nat,
    depth: nat,
)
    ensures
        increment_count(increment_count(accumulator, left, kind, depth), right, kind, depth)
            == increment_count(
                increment_count(accumulator, right, kind, depth),
                left,
                kind,
                depth,
            ),
{
    if contribution_matches(left, kind, depth) {
        if contribution_matches(right, kind, depth) {
            assert(increment_count(accumulator, left, kind, depth) == accumulator + 1nat);
            assert(increment_count(accumulator, right, kind, depth) == accumulator + 1nat);
        } else {
            assert(increment_count(accumulator, left, kind, depth) == accumulator + 1nat);
            assert(increment_count(accumulator, right, kind, depth) == accumulator);
        }
    } else if contribution_matches(right, kind, depth) {
        assert(increment_count(accumulator, left, kind, depth) == accumulator);
        assert(increment_count(accumulator, right, kind, depth) == accumulator + 1nat);
    } else {
        assert(increment_count(accumulator, left, kind, depth) == accumulator);
        assert(increment_count(accumulator, right, kind, depth) == accumulator);
    }
}

proof fn lemma_fold_count_from_equals_accumulator_plus_matching_count(
    contributions: Seq<Contribution>,
    kind: nat,
    depth: nat,
    accumulator: nat,
)
    ensures
        fold_count_from(contributions, kind, depth, accumulator)
            == accumulator + matching_count(contributions, kind, depth),
    decreases contributions.len()
{
    if contributions.len() == 0 {
        assert(matching_count(contributions, kind, depth) == 0nat);
    } else {
        let first = contributions[0];
        let rest = contributions.drop_first();
        lemma_fold_count_from_equals_accumulator_plus_matching_count(
            rest,
            kind,
            depth,
            increment_count(accumulator, first, kind, depth),
        );

        if contribution_matches(first, kind, depth) {
            assert(increment_count(accumulator, first, kind, depth) == accumulator + 1nat);
            assert(matching_count(contributions, kind, depth)
                == 1nat + matching_count(rest, kind, depth));
            assert((accumulator + 1nat) + matching_count(rest, kind, depth)
                == accumulator + matching_count(contributions, kind, depth));
        } else {
            assert(increment_count(accumulator, first, kind, depth) == accumulator);
            assert(matching_count(contributions, kind, depth)
                == matching_count(rest, kind, depth));
        }
    }
}

proof fn lemma_fold_count_is_permutation_invariant(
    original: Seq<Contribution>,
    permuted: Seq<Contribution>,
    kind: nat,
    depth: nat,
)
    requires
        same_contribution_multiset(original, permuted),
    ensures
        fold_count(original, kind, depth) == fold_count(permuted, kind, depth),
{
    lemma_fold_count_from_equals_accumulator_plus_matching_count(original, kind, depth, 0nat);
    lemma_fold_count_from_equals_accumulator_plus_matching_count(permuted, kind, depth, 0nat);
    assert(matching_count(original, kind, depth) == matching_count(permuted, kind, depth));
}

proof fn lemma_two_contribution_fold_is_order_independent(
    left: Contribution,
    right: Contribution,
    kind: nat,
    depth: nat,
)
    ensures
        fold_count(seq![left, right], kind, depth) == fold_count(seq![right, left], kind, depth),
{
    assert(seq![left, right].len() == 2) by (compute);
    assert(seq![right, left].len() == 2) by (compute);
    assert(seq![left, right][0] == left) by (compute);
    assert(seq![right, left][0] == right) by (compute);
    assert(seq![left, right].drop_first() == seq![right]) by (compute);
    assert(seq![right, left].drop_first() == seq![left]) by (compute);
    assert(seq![right].len() == 1) by (compute);
    assert(seq![left].len() == 1) by (compute);
    assert(seq![right][0] == right) by (compute);
    assert(seq![left][0] == left) by (compute);
    assert(seq![right].drop_first() == Seq::<Contribution>::empty()) by (compute);
    assert(seq![left].drop_first() == Seq::<Contribution>::empty()) by (compute);
    assert(fold_count_from(
        Seq::<Contribution>::empty(),
        kind,
        depth,
        increment_count(increment_count(0nat, left, kind, depth), right, kind, depth),
    ) == increment_count(increment_count(0nat, left, kind, depth), right, kind, depth));
    assert(fold_count_from(
        Seq::<Contribution>::empty(),
        kind,
        depth,
        increment_count(increment_count(0nat, right, kind, depth), left, kind, depth),
    ) == increment_count(increment_count(0nat, right, kind, depth), left, kind, depth));

    assert(fold_count(seq![left, right], kind, depth) == fold_count_from(
        seq![right],
        kind,
        depth,
        increment_count(0nat, left, kind, depth),
    ));
    assert(fold_count(seq![right, left], kind, depth) == fold_count_from(
        seq![left],
        kind,
        depth,
        increment_count(0nat, right, kind, depth),
    ));
    assert(fold_count_from(
        seq![right],
        kind,
        depth,
        increment_count(0nat, left, kind, depth),
    ) == increment_count(increment_count(0nat, left, kind, depth), right, kind, depth));
    assert(fold_count_from(
        seq![left],
        kind,
        depth,
        increment_count(0nat, right, kind, depth),
    ) == increment_count(increment_count(0nat, right, kind, depth), left, kind, depth));
    lemma_increment_count_commutes(0nat, left, right, kind, depth);
}

proof fn lemma_documented_ast_count_examples()
    ensures
        fold_count(
            seq![
                Contribution { kind: 10nat, depth: 0nat },
                Contribution { kind: 11nat, depth: 1nat },
                Contribution { kind: 10nat, depth: 0nat },
            ],
            10nat,
            0nat,
        ) == 2nat,
        fold_count(
            seq![
                Contribution { kind: 11nat, depth: 1nat },
                Contribution { kind: 10nat, depth: 0nat },
            ],
            10nat,
            0nat,
        ) == fold_count(
            seq![
                Contribution { kind: 10nat, depth: 0nat },
                Contribution { kind: 11nat, depth: 1nat },
            ],
            10nat,
            0nat,
        ),
{
    assert(fold_count(
        seq![
            Contribution { kind: 10nat, depth: 0nat },
            Contribution { kind: 11nat, depth: 1nat },
            Contribution { kind: 10nat, depth: 0nat },
        ],
        10nat,
        0nat,
    ) == 2nat) by (compute);

    lemma_two_contribution_fold_is_order_independent(
        Contribution { kind: 11nat, depth: 1nat },
        Contribution { kind: 10nat, depth: 0nat },
        10nat,
        0nat,
    );
}

fn main() {
}

} // verus!
