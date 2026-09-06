//! Positive UI fixture: a plain helper inside `#[cfg(test)] mod tests`.
//!
//! The other `cfg(test)` fixtures put `.expect(...)` inside a `#[test]`
//! function, so they pass on the test attribute and say nothing about whether
//! module ancestry was detected. This helper carries no attribute at all, so
//! it passes only when the `cfg(test)` on its parent module is seen.
#![deny(no_expect_outside_tests)]

#[cfg(test)]
mod tests {
    //! Test-only helpers whose ancestry is the whole point of this fixture.

    fn first_item(items: &[u8]) -> u8 {
        items.first().copied().expect("cfg(test) ancestry permits expect")
    }

    #[test]
    fn uses_the_helper() {
        assert_eq!(first_item(&[1]), 1);
    }
}

fn main() {}
