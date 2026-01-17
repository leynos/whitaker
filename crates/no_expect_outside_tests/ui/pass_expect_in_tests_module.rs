//! Positive UI fixture: allow `.expect(...)` inside a `#[cfg(test)] mod tests` block.
//!
//! This exercises detection of test modules marked with `#[cfg(test)]`,
//! the conventional pattern for unit test modules.
#![deny(no_expect_outside_tests)]

#[cfg(test)]
mod tests {
    #[test]
    fn check_cfg_test_detection() {
        let option = Some("ok");
        // Should be allowed because we're inside a #[cfg(test)] module with #[test]
        option.expect("cfg(test) module with test attribute permits expect");
    }
}

fn main() {}
