//! Shared bounded iteration model for decomposition-advice Kani proofs.

/// Maximum iteration bound explored by propagation proofs.
pub(super) const MAX_ITERATIONS: usize = 3;

/// Generates a symbolic iteration count constrained to `[0, MAX_ITERATIONS]`.
pub(super) fn bounded_iteration_count() -> usize {
    let max_iterations: usize = kani::any();
    kani::assume(max_iterations <= MAX_ITERATIONS);
    max_iterations
}
