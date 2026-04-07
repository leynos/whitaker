// aux-build: tokio.rs
//! Positive UI fixture: allow `.expect(...)` in `#[tokio::test]` functions
//! within a `#[path]`-loaded module with a non-standard name.
//!
//! Regression test for <https://github.com/leynos/whitaker/issues/132>.
#![deny(no_expect_outside_tests)]

extern crate core;
extern crate tokio;

#[path = "pass_expect_in_path_module_tokio_test/service_tests.module"]
mod service_tests;

fn main() {}
