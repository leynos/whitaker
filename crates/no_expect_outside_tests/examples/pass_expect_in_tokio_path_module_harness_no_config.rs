//! Regression example covering `#[tokio::test]` in a `#[path]`-loaded module
//! without extra lint configuration.
//!
//! This keeps the `cfg(test)` ancestor walk as the load-bearing mechanism for
//! file-backed Tokio tests, independent of `additional_test_attributes`.

#[cfg(test)]
#[path = "pass_expect_in_tokio_path_module_harness_no_config/service_tests.module"]
mod service_tests;

fn main() {}
