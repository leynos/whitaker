//! Positive UI fixture: allow `.expect(...)` inside a `mod test` block.
//!
//! This exercises the fallback detection that checks for modules named "test"
//! (singular) in addition to "tests" (plural).
#![deny(no_expect_outside_tests)]

mod test {
    pub fn check_fallback_detection() {
        let option = Some("ok");
        // Should be allowed because we're inside a module named "test"
        option.expect("test module permits expect");
    }
}

fn main() {
    test::check_fallback_detection();
}
