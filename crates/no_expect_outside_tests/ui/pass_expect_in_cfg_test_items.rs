//! Positive UI fixture: allow `.expect(...)` in items that are compiled only
//! under `cfg(test)`, whatever their kind.
//!
//! Run under the `--test` harness, so the `cfg(test)` items exist. A
//! `#[cfg(test)]` function, an inherent method of a `#[cfg(test)]` impl, a
//! provided method of a `#[cfg(test)]` trait, and an impl of that trait are all
//! test-only code (leynos/whitaker#311).
#![deny(no_expect_outside_tests)]

/// A free function compiled only under `cfg(test)`.
#[cfg(test)]
fn test_only_function() -> u32 {
    "7".parse::<u32>()
        .expect("a cfg(test) function may expect")
}

/// A type compiled only under `cfg(test)`.
#[cfg(test)]
struct Fixture;

#[cfg(test)]
impl Fixture {
    /// An inherent method of a `cfg(test)` impl.
    fn build() -> u32 {
        "7".parse::<u32>()
            .expect("a method of a cfg(test) impl may expect")
    }
}

/// A trait compiled only under `cfg(test)`, with a provided method.
#[cfg(test)]
trait Helper {
    /// A provided method of a `cfg(test)` trait.
    fn value(&self) -> u32 {
        "7".parse::<u32>()
            .expect("a provided method of a cfg(test) trait may expect")
    }
}

#[cfg(test)]
impl Helper for Fixture {}

/// A test that uses every item above.
#[cfg(test)]
#[test]
fn uses_the_test_only_items() {
    assert_eq!(test_only_function(), 7);
    assert_eq!(Fixture::build(), 7);
    assert_eq!(Fixture.value(), 7);
}

/// The crate's entry point, which the harness does not run.
fn main() {}
