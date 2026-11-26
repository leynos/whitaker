//! Refinement bridge from sparse feature maps to the positional sequence model.
//!
//! `verus/decomposition_vector_algebra.rs` proves the algebraic properties that
//! the decomposition advice relies on, but it states them over positional
//! `Seq<nat>` values. The shipped implementation in
//! `common/src/decomposition_advice/vector.rs` instead keeps features in a
//! sparse `BTreeMap<String, u64>` and multiplies only the weights of keys
//! present in both maps. Without a stated correspondence, those positional
//! theorems say nothing about the runtime behaviour. This module supplies the
//! correspondence and proves that the positional results describe the sparse
//! representation.
//!
//! [`to_seq`] is the abstraction function. It lifts a sparse map into a
//! positional sequence aligned with a *feature universe*: a duplicate-free
//! sequence of feature keys supplied by the caller. Each position of the result
//! holds that key's weight, or `0` when the map has no entry for the key. This
//! is exactly the "absent entries count as zero" reading that the positional
//! proof file already uses. The runtime builds its universe from `BTreeMap`
//! iteration, which yields the keys in sorted order; sorting is a convenience of
//! the encoder, since every theorem below depends on the universe's membership
//! and uniqueness but never on its order.
//!
//! Feature strings are abstracted as `nat` keys. The runtime keys are `String`
//! values, and the correspondence assumed here is a bijection between the two.
//! That bijection is *not* modelled or proved: it is a modelling assumption
//! about the encoder that builds the maps, and every theorem below is
//! insensitive to how it is chosen, provided distinct features receive distinct
//! keys. The scope of this file is therefore the two mathematical models, not
//! the Rust data structures; `BTreeMap` iteration order and `u64` overflow stay
//! with the unit and Kani suites.
//!
//! The refinement equality holds for any duplicate-free universe that contains
//! every key of both maps. [`lemma_dot_product_refinement`] states that
//! precondition explicitly, because it is what licenses reading
//! [`sum_matching_keys`] as the runtime key-intersection loop: a key present in
//! both maps but missing from the universe would be dropped by the abstraction.
//! The equality itself is order-insensitive algebra, so
//! [`lemma_dot_product_refinement_core`] proves it without the precondition,
//! and the preconditioned lemma derives both facts from it.

use vstd::prelude::*;

// The refinement theorem must speak about the same `dot_product` that the
// positional proof file verifies. Including that file as a module keeps a
// single definition instead of a drifting copy.
#[path = "decomposition_vector_algebra.rs"]
mod positional;

verus! {

// ---------------------------------------------------------------------------
// Sequence foundations
// ---------------------------------------------------------------------------

/// Concatenation adds sequence lengths.
///
/// This restates the `vstd` sequence axiom as a named fact so the refinement
/// proofs can cite the step that a positional prepend grows the vector by one.
proof fn lemma_seq_concat_len(a: Seq<nat>, b: Seq<nat>)
    ensures
        (a + b).len() == a.len() + b.len(),
{
}

/// A subrange has the length implied by its bounds.
proof fn lemma_subrange_len(s: Seq<nat>, low: int, high: int)
    requires
        0 <= low <= high <= s.len(),
    ensures
        s.subrange(low, high).len() == high - low,
{
}

/// Prepending one weight exposes it at index zero and leaves the rest intact.
///
/// Dropping the first element of a prepended sequence recovers the original
/// tail. Without this fact the induction in
/// [`lemma_dot_product_refinement_core`] cannot relate the recursive
/// `dot_product` call to the smaller universe.
proof fn lemma_singleton_concat(weight: nat, rest: Seq<nat>)
    ensures
        (seq![weight] + rest).len() == rest.len() + 1,
        (seq![weight] + rest)[0] == weight,
        (seq![weight] + rest).subrange(1, (seq![weight] + rest).len() as int) == rest,
{
    lemma_seq_concat_len(seq![weight], rest);
}

// ---------------------------------------------------------------------------
// The abstraction function
// ---------------------------------------------------------------------------

/// Returns the weight a sparse map assigns to `key`, or zero when absent.
///
/// The runtime dot product treats a missing key as contributing nothing, which
/// is the same as carrying weight zero.
spec fn feature_weight(map: Map<nat, nat>, key: nat) -> nat {
    if map.contains_key(key) {
        map[key]
    } else {
        0nat
    }
}

/// Lifts a sparse map into a positional sequence over `universe`.
///
/// This is the abstraction function relating the runtime `BTreeMap` state to
/// the positional `Seq<nat>` model. Position `index` holds
/// `feature_weight(map, universe[index])`, so two maps compared over the same
/// universe line up by feature key rather than by iteration order.
///
/// The duplicate-free recommendation is a precondition of the intended use, not
/// of the definition: a universe that repeats a key simply repeats that
/// weight. Callers that want one position per feature supply
/// `universe.no_duplicates()`.
spec fn to_seq(map: Map<nat, nat>, universe: Seq<nat>) -> Seq<nat>
    recommends
        universe.no_duplicates(),
    decreases universe.len(),
{
    if universe.len() == 0 {
        seq![]
    } else {
        let key = universe[0];
        seq![feature_weight(map, key)]
            + to_seq(map, universe.subrange(1, universe.len() as int))
    }
}

/// Returns one key's contribution to the runtime key-intersection loop.
///
/// The runtime multiplies a pair of weights only when both maps contain the
/// key; every other key contributes zero.
spec fn matching_term(left: Map<nat, nat>, right: Map<nat, nat>, key: nat) -> nat {
    if left.contains_key(key) && right.contains_key(key) {
        left[key] * right[key]
    } else {
        0nat
    }
}

/// Sums the runtime intersections over `universe`.
///
/// This is the specification of the loop in `dot_product` in
/// `common/src/decomposition_advice/vector.rs`, which walks the entries of the
/// smaller map and adds `left_weight * right_weight` for keys the other map
/// also holds. Iterating a `BTreeMap` by key produces the same terms in a
/// different order, so this sequence-shaped sum is the runtime value whenever
/// `universe` contains every key of both maps.
spec fn sum_matching_keys(
    left: Map<nat, nat>,
    right: Map<nat, nat>,
    universe: Seq<nat>,
) -> nat
    decreases universe.len(),
{
    if universe.len() == 0 {
        0nat
    } else {
        let key = universe[0];
        matching_term(left, right, key)
            + sum_matching_keys(left, right, universe.subrange(1, universe.len() as int))
    }
}

/// The abstraction function preserves the length of its universe.
///
/// The positional `dot_product` recommends equal operand lengths. Both maps are
/// lifted over the same universe, so this lemma is what discharges that
/// recommendation at every refinement call site.
proof fn lemma_to_seq_len(map: Map<nat, nat>, universe: Seq<nat>)
    requires
        universe.no_duplicates(),
    ensures
        to_seq(map, universe).len() == universe.len(),
    decreases universe.len(),
{
    if universe.len() > 0 {
        let rest = universe.subrange(1, universe.len() as int);
        lemma_to_seq_len(map, rest);
        lemma_singleton_concat(feature_weight(map, universe[0]), to_seq(map, rest));
    }
}

/// Position `index` of the abstraction holds that feature's weight.
///
/// This is the per-index reading of [`to_seq`]: the lifted vector carries no
/// information that the map did not, and it drops no weight for a key the
/// universe contains.
proof fn lemma_to_seq_index(map: Map<nat, nat>, universe: Seq<nat>, index: int)
    requires
        universe.no_duplicates(),
        0 <= index < universe.len(),
    ensures
        to_seq(map, universe)[index] == feature_weight(map, universe[index]),
    decreases universe.len(),
{
    let rest = universe.subrange(1, universe.len() as int);
    let head = feature_weight(map, universe[0]);
    lemma_to_seq_len(map, rest);
    lemma_subrange_len(universe, 1, universe.len() as int);
    lemma_singleton_concat(head, to_seq(map, rest));
    if index == 0 {
    } else {
        lemma_to_seq_index(map, rest, index - 1);
        assert(rest[index - 1] == universe[index]);
    }
}

// ---------------------------------------------------------------------------
// Refinement
// ---------------------------------------------------------------------------

/// The positional dot product of two lifted maps is the runtime intersection
/// sum.
///
/// This is the algebraic core of the refinement bridge, proved by induction on
/// the universe. It deliberately carries no completeness precondition: the
/// equality relates [`to_seq`] and [`sum_matching_keys`] over whatever universe
/// the caller chose, and a universe that omits a key omits it from both sides.
/// [`lemma_dot_product_refinement`] adds the precondition that makes the sum a
/// faithful model of the runtime loop.
proof fn lemma_dot_product_refinement_core(
    left: Map<nat, nat>,
    right: Map<nat, nat>,
    universe: Seq<nat>,
)
    requires
        universe.no_duplicates(),
    ensures
        positional::dot_product(to_seq(left, universe), to_seq(right, universe))
            == sum_matching_keys(left, right, universe),
    decreases universe.len(),
{
    if universe.len() == 0 {
    } else {
        let key = universe[0];
        let rest = universe.subrange(1, universe.len() as int);
        let left_weight = feature_weight(left, key);
        let right_weight = feature_weight(right, key);
        lemma_to_seq_len(left, rest);
        lemma_to_seq_len(right, rest);
        lemma_singleton_concat(left_weight, to_seq(left, rest));
        lemma_singleton_concat(right_weight, to_seq(right, rest));
        lemma_dot_product_refinement_core(left, right, rest);
        assert(
            positional::dot_product(to_seq(left, universe), to_seq(right, universe))
                == left_weight * right_weight
                    + positional::dot_product(to_seq(left, rest), to_seq(right, rest))
        );
        assert(matching_term(left, right, key) == left_weight * right_weight);
    }
}

/// The runtime dot product refines the positional dot product.
///
/// This is the bridge the module exists for. Given a duplicate-free feature
/// universe that contains every key of both maps, the positional `dot_product`
/// of the two lifted vectors equals [`sum_matching_keys`], the specification of
/// the runtime key-intersection loop. The completeness preconditions are what
/// make that equality a statement about the *runtime* sum: [`to_seq`] drops
/// keys the universe does not mention, so a contributing key outside the
/// universe would silently vanish from the abstraction.
///
/// The second postcondition records that consequence directly: every key
/// present in both maps is present in the universe, so no runtime term is
/// unaccounted for.
proof fn lemma_dot_product_refinement(
    left: Map<nat, nat>,
    right: Map<nat, nat>,
    universe: Seq<nat>,
)
    requires
        universe.no_duplicates(),
        forall|key: nat| #![trigger left.contains_key(key)] left.contains_key(key)
            ==> universe.contains(key),
        forall|key: nat| #![trigger right.contains_key(key)] right.contains_key(key)
            ==> universe.contains(key),
    ensures
        positional::dot_product(to_seq(left, universe), to_seq(right, universe))
            == sum_matching_keys(left, right, universe),
        forall|key: nat|
            #![trigger left.contains_key(key), right.contains_key(key)]
            left.contains_key(key) && right.contains_key(key) ==> universe.contains(key),
{
    lemma_dot_product_refinement_core(left, right, universe);
}

/// The runtime squared norm refines the positional squared norm.
///
/// The runtime computes `norm_squared` as the sum of squared weights over the
/// map, which is the runtime dot product of the map with itself. Lifting that
/// through the abstraction gives the positional `norm_squared`, so the positional
/// norm theorems apply to the shipped implementation under the same
/// completeness precondition as the dot-product bridge.
proof fn lemma_norm_squared_refinement(map: Map<nat, nat>, universe: Seq<nat>)
    requires
        universe.no_duplicates(),
        forall|key: nat| #![trigger map.contains_key(key)] map.contains_key(key)
            ==> universe.contains(key),
    ensures
        positional::norm_squared(to_seq(map, universe)) == sum_matching_keys(map, map, universe),
{
    lemma_dot_product_refinement(map, map, universe);
}

/// A worked instance of the refinement equality.
///
/// The universe contains a key shared by both maps and one key private to each,
/// mirroring the shared-feature scenario in
/// `common/tests/features/decomposition_vector_algebra.feature`. Only the
/// shared key contributes, so the positional dot product is `6 * 6 = 36`.
proof fn lemma_refinement_example()
    ensures
        positional::dot_product(
            to_seq(map![0nat => 6nat, 1nat => 2nat], seq![0nat, 1nat, 2nat]),
            to_seq(map![0nat => 6nat, 2nat => 5nat], seq![0nat, 1nat, 2nat]),
        ) == 36nat,
{
    lemma_to_seq_len(map![0nat => 6nat, 1nat => 2nat], seq![0nat, 1nat, 2nat]);
    lemma_to_seq_len(map![0nat => 6nat, 2nat => 5nat], seq![0nat, 1nat, 2nat]);
    lemma_dot_product_refinement(
        map![0nat => 6nat, 1nat => 2nat],
        map![0nat => 6nat, 2nat => 5nat],
        seq![0nat, 1nat, 2nat],
    );
    assert(sum_matching_keys(
        map![0nat => 6nat, 1nat => 2nat],
        map![0nat => 6nat, 2nat => 5nat],
        seq![0nat, 1nat, 2nat],
    ) == 36nat) by (compute);
}

fn main() {
}

} // verus!