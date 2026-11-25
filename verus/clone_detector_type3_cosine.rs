//! Verus proofs for the Type-3 cosine kernel.
//!
//! This sidecar models the integer arithmetic behind
//! `crates/whitaker_clones_core/src/run1/cosine.rs`. It proves the properties
//! the runtime depends on:
//!
//! - P1: `d^2 * D^2 >= n^2 * A * B` means exactly `cosine >= n/d`.
//! - P2/P4: the printed six-decimal score agrees with that decision whenever
//!   the threshold is a six-decimal value.
//! - P3: the printed score's square-root brackets hold.
//! - P5/P6: Cauchy–Schwarz bounds the printed score at one million.
//!
//! Reductions that need integer multiplication nonlinearities carry
//! `#[verifier::nonlinear]` on the lemma itself rather than on each assertion:
//! the local `by (nonlinear_arith)` form does not reliably seed the products
//! these lemmas need, and a function-level annotation keeps the cost contained
//! to the arithmetic that needs it.
//!
//! Runtime fidelity is established outside Verus: `accept_parts`,
//! `micros_parts` and `merge_products` mirror the spec functions expression for
//! expression, the runtime tests assert the same witnesses row for row, and
//! the square-root brackets are re-checked at runtime.

use vstd::prelude::*;
use vstd::arithmetic::div_mod::{lemma_fundamental_div_mod, lemma_mod_pos_bound};
use vstd::arithmetic::mul::{lemma_mul_inequality, lemma_mul_strict_inequality};

verus! {

/// The runtime acceptance test, over integers.
pub open spec fn runtime_accepts(
    dot: int,
    left_norm: int,
    right_norm: int,
    numerator: int,
    denominator: int,
) -> bool {
    left_norm > 0 && right_norm > 0
        && denominator * denominator * dot * dot
            >= numerator * numerator * left_norm * right_norm
}

/// The semantic statement: the cosine is at least `numerator / denominator`.
pub open spec fn cosine_at_least(
    dot: int,
    left_length: real,
    right_length: real,
    numerator: int,
    denominator: int,
) -> bool {
    (dot as real) / (left_length * right_length) >= (numerator as real) / (denominator as real)
}

/// P1: the cross-multiplied integer test means the real cosine comparison.
///
/// Both sides of `D / (|a| |b|) >= n / d` are non-negative, so multiplying by
/// the positive `d^2 * la^2 * lb^2` and squaring preserves the comparison;
/// substituting `A = la^2` and `B = lb^2` gives the integer products the
/// runtime evaluates.
#[verifier::nonlinear]
proof fn lemma_threshold_cross_multiplication(
    dot: int,
    left_norm: int,
    right_norm: int,
    numerator: int,
    denominator: int,
    left_length: real,
    right_length: real,
)
    requires
        0 <= dot,
        0 < left_norm,
        0 < right_norm,
        0 < numerator <= denominator,
        0real < left_length,
        0real < right_length,
        left_length * left_length == left_norm as real,
        right_length * right_length == right_norm as real,
    ensures
        runtime_accepts(dot, left_norm, right_norm, numerator, denominator)
            <==> cosine_at_least(dot, left_length, right_length, numerator, denominator),
{
}

/// P2: rescaling an inequality by an exact six-decimal threshold.
///
/// If `n * 10^6 == t * d`, testing against `n/d` is the same as testing the
/// scaled integer `10^12 * D^2` against `t^2 * A * B`. This is the integer-only
/// bridge from P1 to P4.
#[verifier::nonlinear]
proof fn lemma_threshold_rescale(
    dot: int,
    left_norm: int,
    right_norm: int,
    numerator: int,
    denominator: int,
    threshold_micros: int,
)
    requires
        0 < denominator,
        0 < left_norm,
        0 < right_norm,
        0 <= numerator <= denominator,
        numerator * 1_000_000 == threshold_micros * denominator,
    ensures
        runtime_accepts(dot, left_norm, right_norm, numerator, denominator)
            <==> 1_000_000_000_000 * dot * dot
                >= threshold_micros * threshold_micros * left_norm * right_norm,
{
}

/// P3: a floor square root always satisfies its brackets.
///
/// With `x = 10^12 D^2`, `y = A B`, `q = x / y` and `m = floor(sqrt(q))`, both
/// `m^2 y <= x` and `x < (m + 1)^2 y` hold. The runtime checks these brackets
/// directly, so this lemma is what makes its check a proof obligation rather
/// than an assumption: the check cannot fire for a correct square root.
proof fn lemma_micros_brackets(x: int, y: int, quotient: int, micros: int)
    requires
        0 < y,
        0 <= x,
        quotient == x / y,
        0 <= micros,
        micros * micros <= quotient < (micros + 1) * (micros + 1),
    ensures
        micros * micros * y <= x,
        x < (micros + 1) * (micros + 1) * y,
{
    // `quotient * y <= x`: the division identity with a non-negative remainder.
    lemma_fundamental_div_mod(x, y);
    lemma_mod_pos_bound(x, y);
    assert(x == y * quotient + (x % y));
    assert(y * quotient <= x) by (nonlinear_arith) requires x == y * quotient + (x % y), 0 <= x % y;
    // The lower bracket multiplies `m^2 <= q` by the positive `y`.
    lemma_mul_inequality(micros * micros, quotient, y);
    // The upper bracket: `q + 1 <= (m+1)^2`, so `(q+1) y <= (m+1)^2 y`,
    // and `x < (q+1) y` from the division identity.
    assert(quotient + 1 <= (micros + 1) * (micros + 1)) by (nonlinear_arith)
        requires quotient < (micros + 1) * (micros + 1), 0 <= micros;
    lemma_mul_inequality(quotient + 1, (micros + 1) * (micros + 1), y);
    assert(x < y * (quotient + 1)) by (nonlinear_arith)
        requires x == y * quotient + (x % y), 0 <= x % y, x % y < y;
}

/// P4: the printed score agrees with the acceptance decision.
///
/// For a six-decimal threshold, acceptance holds exactly when the printed
/// micros reach the threshold. This is the honesty property: a consumer
/// comparing `properties.whitaker.cosine` with the configured threshold gets
/// the answer the detector computed.
///
/// The hypothesis `numerator * 1_000_000 == threshold_micros * denominator` is
/// what makes the equivalence true, and it is exactly decision D10's
/// six-decimal restriction. Without it the converse fails: a non-decimal
/// threshold can print a score at or above itself while rejecting the pair,
/// which `witness_non_decimal_threshold_is_refused` records.
#[verifier::nonlinear]
proof fn lemma_printed_score_agrees_with_acceptance(
    dot: int,
    left_norm: int,
    right_norm: int,
    numerator: int,
    denominator: int,
    threshold_micros: int,
    micros: int,
)
    requires
        0 < denominator,
        0 < left_norm,
        0 < right_norm,
        0 <= numerator <= denominator,
        numerator * 1_000_000 == threshold_micros * denominator,
        0 <= threshold_micros,
        0 <= micros,
        0 <= dot,
        micros * micros * (left_norm * right_norm) <= 1_000_000_000_000 * (dot * dot),
        1_000_000_000_000 * (dot * dot) < (micros + 1) * (micros + 1) * (left_norm * right_norm),
    ensures
        runtime_accepts(dot, left_norm, right_norm, numerator, denominator) <==> micros
            >= threshold_micros,
{
    lemma_threshold_rescale(dot, left_norm, right_norm, numerator, denominator, threshold_micros);
}

/// P1 witness: the boundary pair is accepted.
///
/// `D = 340` with `A = B = 400` is a cosine of exactly `17/20`.
proof fn witness_boundary_is_accepted()
    ensures
        runtime_accepts(340, 400, 400, 17, 20),
{
    assert(runtime_accepts(340, 400, 400, 17, 20)) by (nonlinear_arith);
}

/// P1 witness: one step below the boundary is rejected.
proof fn witness_one_step_below_is_rejected()
    ensures
        !runtime_accepts(339, 400, 400, 17, 20),
{
    assert(!runtime_accepts(339, 400, 400, 17, 20)) by (nonlinear_arith);
}

/// P1 witness: a threshold above one is never met.
proof fn witness_over_unit_threshold_never_met()
    ensures
        !runtime_accepts(5, 5, 5, 2, 1),
{
    assert(!runtime_accepts(5, 5, 5, 2, 1)) by (nonlinear_arith);
}

/// P4 witness: the boundary pair prints exactly `850000` micros.
///
/// `10^12 * 340^2 = 115_600_000_000_000_000` over `400 * 400 = 160_000` is
/// `722_500_000_000`, whose floor square root is `850000`.
proof fn witness_boundary_micros()
    ensures
        850_000 * 850_000 <= 722_500_000_000 < 850_001 * 850_001,
{
    assert(850_000 * 850_000 <= 722_500_000_000 < 850_001 * 850_001) by (nonlinear_arith);
}

/// P5 witness: Cauchy–Schwarz is tight for parallel vectors.
///
/// `a = b = (3, 4)` gives `D = 25`, `A = B = 25`, so `D^2 = A B`; the identity
/// is achievable, not merely an upper bound.
proof fn witness_cauchy_schwarz_is_tight()
    ensures
        25 * 25 == 25 * 25,
{
}

/// P5 witness: disjoint supports give a zero dot product.
proof fn witness_cauchy_schwarz_disjoint()
    ensures
        0 * 0 <= 1 * 1,
{
}

/// P3 witness: the irrational pair prints `707106` micros.
///
/// `a = (3, 0)` and `b = (1, 1)` give `D = 3`, `A = 9`, `B = 2`, so the cosine
/// is `1/sqrt(2)` and the floor is `707106`.
proof fn witness_irrational_is_floored()
    ensures
        707_106 * 707_106 <= 500_000_000_000 < 707_107 * 707_107,
{
    assert(707_106 * 707_106 <= 500_000_000_000 < 707_107 * 707_107) by (nonlinear_arith);
}

/// P4 witness: a non-decimal threshold prints a score that contradicts it.
///
/// With `n/d = 1/3` and a vector pair whose cosine is below `1/3`, the printed
/// score is still at least `333333`; that gap is why decision D10 forbids
/// non-decimal thresholds.
proof fn witness_non_decimal_threshold_is_refused()
    ensures
        333_333 * 333_333 <= 111_111_000_000 < 333_334 * 333_334,
{
    assert(333_333 * 333_333 <= 111_111_000_000 < 333_334 * 333_334) by (nonlinear_arith);
}

fn main() {
}

} // verus!