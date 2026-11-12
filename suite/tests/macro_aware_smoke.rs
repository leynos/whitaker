//! End-to-end smoke test for the experimental macro-aware lint in the suite.
//!
//! This target only selects the feature state and delegates to the harness in
//! `support/macro_aware_smoke.rs`. See that module for how the suite cdylib is
//! built, staged, and exercised through a real Dylint driver.
#![cfg(feature = "dylint-driver")]

#[path = "support/macro_aware_smoke.rs"]
mod support;

/// Prove Dylint loads the suite and runs the macro-aware lint as configured.
#[test]
fn suite_runs_the_macro_aware_lint_in_a_real_driver() { support::run_smoke(); }
