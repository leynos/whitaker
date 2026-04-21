//! Positive UI fixture: allow `.expect(...)` inside a file-backed
//! `#[cfg(test)] mod service_tests;` declaration.
//!
//! This keeps file-backed module ancestry covered independently of harness
//! handling and non-standard module names.
#![deny(no_expect_outside_tests)]

#[cfg(test)]
#[path = "pass_expect_in_file_backed_test_module/auxiliary/pass_expect_in_file_backed_test_module_service_tests.rs"]
mod service_tests;

fn main() {}
