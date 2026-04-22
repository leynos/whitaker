//! Negative UI fixture: file-backed `#[cfg(test)]` ancestry must not leak into
//! ordinary functions.
//!
//! The auxiliary `service_tests` module is still test-only code, but `main`
//! must continue to trigger the lint.
#![deny(no_expect_outside_tests)]

#[cfg(test)]
#[path = "fail_expect_in_file_backed_non_test_fn/auxiliary/fail_expect_in_file_backed_non_test_fn_service_tests.rs"]
mod service_tests;

fn main() {
    let value = Some("value");
    let _ = value.expect("file-backed cfg(test) ancestry must not leak into main");
}
