//! End-to-end smoke test for the experimental macro-aware lint in the suite.
//!
//! This target only selects the feature state and delegates to the harness in
//! `support/macro_aware_smoke.rs`. See that module for how the suite cdylib is
//! built, staged, and exercised through a real Dylint driver.
//!
//! # Platforms
//!
//! The harness runs `cargo dylint`, which this repository installs only from
//! the digest-verified upstream release archives; those cover Linux alone, and
//! upstream publishes none for Windows. Windows therefore skips this test
//! rather than dropping it: what it proves is fixed by the pinned compiler, so
//! the Linux lanes carry the obligation and the Windows lane only has to keep
//! the test type-checked.
#![cfg(feature = "dylint-driver")]

#[path = "support/macro_aware_smoke.rs"]
mod support;

/// Prove Dylint loads the suite and runs the macro-aware lint as configured.
#[test]
#[cfg_attr(
    windows,
    ignore = "upstream publishes no Windows archive for the pinned cargo-dylint, and this \
              repository installs only digest-verified prebuilt archives; the Linux lanes run \
              this smoke test"
)]
fn suite_runs_the_macro_aware_lint_in_a_real_driver() { support::run_smoke(); }
