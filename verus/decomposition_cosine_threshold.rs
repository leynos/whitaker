use vstd::prelude::*;

verus! {

spec fn runtime_cross_multiplied_threshold(dot: int, left_norm: int, right_norm: int) -> bool {
    25 * dot * dot >= left_norm * right_norm
}

spec fn cosine_at_least_one_fifth(dot: int, left_length: real, right_length: real) -> bool {
    (dot as real) / (left_length * right_length) >= 1real / 5real
}

spec fn runtime_algorithm_returns_true(dot: int, left_norm: int, right_norm: int) -> bool {
    if dot == 0 {
        false
    } else if left_norm == 0 || right_norm == 0 {
        false
    } else {
        runtime_cross_multiplied_threshold(dot, left_norm, right_norm)
    }
}

#[verifier::nonlinear]
proof fn lemma_cross_multiplied_threshold_matches_cosine(
    dot: int,
    left_norm: int,
    right_norm: int,
    left_length: real,
    right_length: real,
)
    requires
        0 <= dot,
        0real < left_length,
        0real < right_length,
        left_length * left_length == left_norm as real,
        right_length * right_length == right_norm as real,
    ensures
        runtime_cross_multiplied_threshold(dot, left_norm, right_norm)
            <==> cosine_at_least_one_fifth(dot, left_length, right_length),
{
}

proof fn lemma_zero_norms_short_circuit(dot: int, left_norm: int, right_norm: int)
    requires
        left_norm == 0 || right_norm == 0,
    ensures
        !runtime_algorithm_returns_true(dot, left_norm, right_norm),
{
}

proof fn lemma_zero_dot_short_circuits(left_norm: int, right_norm: int)
    ensures
        !runtime_algorithm_returns_true(0, left_norm, right_norm),
{
}

proof fn lemma_exact_boundary_is_accepted()
    ensures
        runtime_algorithm_returns_true(1, 25, 1),
        runtime_cross_multiplied_threshold(1, 25, 1),
        cosine_at_least_one_fifth(1, 5real, 1real),
{
    lemma_cross_multiplied_threshold_matches_cosine(1, 25, 1, 5real, 1real);
}

fn main() {
}

} // verus!
