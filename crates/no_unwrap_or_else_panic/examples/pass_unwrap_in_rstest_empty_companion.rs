//! Edge-case fixture: companion module present but empty.
//!
//! `collect_rstest_companion_test_functions` must not trip the lint on the
//! parent function even when the sibling module carries no harness items.

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

#[cfg(test)]
mod rstest_empty_companion_subject {}

fn main() {}
