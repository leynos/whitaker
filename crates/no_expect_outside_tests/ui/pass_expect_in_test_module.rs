//! Positive UI fixture: allow `.expect(...)` inside a `#[cfg(test)] mod test` block.
//!
//! This exercises detection of test modules marked with `#[cfg(test)]`.
#![deny(no_expect_outside_tests)]

#[cfg(test)]
mod test {
    #[test]
    fn check_cfg_test_detection() {
        let option = Some("ok");
        // Should be allowed because we're inside a #[cfg(test)] module with #[test]
        option.expect("cfg(test) module with test attribute permits expect");
    }
}

fn main() {}
