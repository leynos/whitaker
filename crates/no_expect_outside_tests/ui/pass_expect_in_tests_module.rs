//! Positive UI fixture: allow `.expect(...)` inside a `mod tests` block.
//!
//! This exercises the fallback detection that checks for modules named "tests"
//! (plural), the conventional name for unit test modules.
#![deny(no_expect_outside_tests)]

mod tests {
    pub fn check_fallback_detection() {
        let option = Some("ok");
        // Should be allowed because we're inside a module named "tests"
        option.expect("tests module permits expect");
    }
}

fn main() {
    tests::check_fallback_detection();
}
