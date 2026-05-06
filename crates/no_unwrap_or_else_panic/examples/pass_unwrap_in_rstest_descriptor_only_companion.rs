//! Edge-case fixture: companion module with descriptor constant but no test function.

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
fn rstest_descriptor_only_subject(value: i32) {
    let _ = Some(value).unwrap_or_else(|| panic!("descriptor only {value}"));
}

/// Descriptor-only companion module used as an edge-case fixture.
///
/// Exposes `RSTEST_HARNESS_DESCRIPTOR` but contains no `#[test]` function.
/// Validates that `collect_rstest_companion_test_functions` treats a
/// descriptor constant alone as sufficient evidence of rstest synthesis.
#[cfg(test)]
mod rstest_descriptor_only_subject {
    pub const RSTEST_HARNESS_DESCRIPTOR: &str = "case_1";

    // No `#[test]` here; anonymous const keeps the descriptor referenced for `dead_code` checks.
    const _: usize = RSTEST_HARNESS_DESCRIPTOR.len();
}

fn main() {}
