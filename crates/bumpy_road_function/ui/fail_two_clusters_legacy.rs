//! UI fixture that should trigger the bumpy road lint.
//!
//! This legacy variant keeps the two conditional clusters inline, without helper functions.
#![expect(dead_code, reason = "UI test fixture; functions are analysed but not invoked")]

/// Produces a value with two separated conditional clusters (legacy layout).
///
/// ```ignore
/// assert_eq!(bumpy(2), 5);
/// ```
pub fn bumpy(input: i32) -> i32 {
    let mut total = 0;

    if input > 0
        && input < 100
        && input != 5
        && input != 7
        && input != 9
        && input != 11
    {
        if input % 2 == 0 {
            total += 1;
        }
        total += 2;
    }

    total += input;

    if input > 1000
        && input < 2000
        && input != 1500
        && input != 1750
        && input != 1800
        && input != 1900
    {
        if input % 3 == 0 {
            total += 3;
        }
        total += 4;
    }

    total
}

fn dead_code_fixture_marker() {}

fn main() {}
