//! Positive UI fixture: allow `.expect(...)` in helper functions that live in a
//! file-backed `#[cfg(test)] mod` companion module.
//!
//! The helpers are not `#[test]` functions, so only the module's `cfg(test)`
//! ancestry marks them as test-only code (leynos/whitaker#311).
#![deny(no_expect_outside_tests)]

#[cfg(test)]
#[path = "pass_expect_in_file_backed_test_helper/auxiliary/pass_expect_in_file_backed_test_helper_companion.rs"]
mod companion;

/// The crate's entry point, which the harness does not run.
fn main() {}
