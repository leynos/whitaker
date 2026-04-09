//! Negative UI fixture: handwritten function with companion test module
//! should not suppress the lint.
#![deny(no_expect_outside_tests)]

// A regular parse function (NOT annotated with #[rstest])
fn parse() -> &'static str {
    let value = Some("data");
    value.expect("should have value")
}

// A test module that happens to have the same name
mod parse {
    #[test]
    fn test_basic() {
        assert!(true);
    }
}

fn main() {
    parse();
}
