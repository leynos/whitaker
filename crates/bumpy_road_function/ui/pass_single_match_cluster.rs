//! UI fixture that should *not* trigger the bumpy road lint.
//!
//! This example uses a match expression where all conditional complexity is
//! concentrated in a single arm, forming only one contiguous cluster.
#![expect(dead_code, reason = "UI test fixture; functions are analysed but not invoked")]

/// Categorises the input with a single cluster of conditional logic.
///
/// ```ignore
/// assert_eq!(categorise(42), "medium");
/// ```
pub fn categorise(input: i32) -> &'static str {
    match input {
        0 => "zero",
        n if n > 0 && n < 10 && n != 5 && n != 7 => {
            if n % 2 == 0 {
                "small even"
            } else {
                "small odd"
            }
        }
        _ => "other",
    }
}

fn dead_code_fixture_marker() {}

fn main() {}
