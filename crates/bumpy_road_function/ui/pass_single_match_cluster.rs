//! UI fixture that should *not* trigger the bumpy road lint.
//!
//! This example uses a match expression where all conditional complexity is
//! concentrated in a single arm, forming only one contiguous cluster.

mod fixture {
    /// Returns `true` when `n` falls in the small eligible range,
    /// i.e. positive, below ten, and neither five nor seven.
    fn is_small_eligible(n: i32) -> bool {
        n > 0 && n < 10 && n != 5 && n != 7
    }

    /// Categorises the input with a single cluster of conditional logic.
    ///
    /// ```ignore
    /// assert_eq!(categorise(42), "other");
    /// ```
    pub fn categorise(input: i32) -> &'static str {
        match input {
            0 => "zero",
            n if is_small_eligible(n) => {
                if n % 2 == 0 {
                    "small even"
                } else {
                    "small odd"
                }
            }
            _ => "other",
        }
    }

    pub fn dead_code_fixture_marker() {}
}

fn main() {
    let _ = fixture::categorise(0);
    let _ = fixture::categorise(4);
    let _ = fixture::categorise(99);
    fixture::dead_code_fixture_marker();
}
