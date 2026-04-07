//! Regression example covering `#[tokio::test]` inside a `#[path]`-loaded
//! module with a non-standard name, compiled under `--test` harness.
//!
//! This validates that the harness descriptor fallback correctly recognises
//! test functions inside `#[path]`-loaded modules, not just at the crate root.
//!
//! Regression test for <https://github.com/leynos/whitaker/issues/132>.

#[path = "path_module_harness_support/service_tests.rs"]
mod service_tests;

fn main() {}
