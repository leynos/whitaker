//! Regression example covering `#[tokio::test]` in a `#[path]`-loaded module.
//!
//! The fixture support directory supplies the file-backed `service_tests`
//! module so the test runs under `rustc --test` with the same layout reported
//! in issue #132.

#[cfg(test)]
#[path = "pass_expect_in_tokio_path_module_harness/service_tests.module"]
mod service_tests;

fn main() {}
