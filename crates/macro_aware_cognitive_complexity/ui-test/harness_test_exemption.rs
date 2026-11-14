//! UI fixture for a `--test` build, where the harness consumes `#[test]`.
//!
//! In a `--test` build rustc removes the authored `#[test]` attribute during
//! expansion and leaves a generated descriptor beside the function, so an
//! attribute scan alone cannot identify a test. The configured limit is one:
//! both functions below score above it, and only the test must stay silent.

/// Branch above the configured limit without being a test.
fn ordinary_above_limit(value: Option<u8>) -> u8 {
    match value {
        Some(inner) if inner > 0 => inner,
        Some(_) => 0,
        None => 1,
    }
}

/// Branch above the configured limit while being a harness test.
#[test]
fn harness_test_above_limit() {
    if ordinary_above_limit(Some(3)) == 3 {
        assert_eq!(ordinary_above_limit(None), 1);
    }
    if ordinary_above_limit(Some(0)) == 0 {
        assert_eq!(ordinary_above_limit(None), 1);
    }
}