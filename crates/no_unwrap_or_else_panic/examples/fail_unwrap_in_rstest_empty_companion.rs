//! Negative edge-case fixture: empty companion module must not exempt parent.
//!
//! An empty same-named sibling module contains no rstest synthesis evidence.
//! `collect_rstest_companion_test_functions` must not mark the parent function
//! as test-context, so the lint fires on `unwrap_or_else(|| panic!(…))`.

#![cfg_attr(
    test,
    feature(rustc_private),
    allow(unknown_lints),
    deny(no_unwrap_or_else_panic)
)]

#[cfg(test)]
#[expect(
    dead_code,
    reason = "fixture body is exercised by the Dylint UI compile only"
)]
fn rstest_empty_companion_subject(value: i32) {
    let _ = Some(value).unwrap_or_else(|| panic!("empty companion {value}"));
}

/// Empty sibling module used as a negative fixture.
///
/// An empty module contains no rstest synthesis evidence; it must not be
/// treated as a companion by `collect_rstest_companion_test_functions`.
/// The parent function remains outside test-context, so the lint fires.
#[cfg(test)]
mod rstest_empty_companion_subject {}

fn main() {}
