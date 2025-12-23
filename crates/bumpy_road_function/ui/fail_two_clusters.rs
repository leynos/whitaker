//! UI fixture that should trigger the bumpy road lint.
//!
//! This refactored variant still contains two separated conditional clusters.
#![expect(dead_code, reason = "UI test fixture; functions are analysed but not invoked")]

/// Applies conditional logic for the low input range (0-100) and returns an accumulated total.
fn process_low_range(input: i32) -> i32 {
    if input > 0 && input < 100 && input != 5 && input != 7 && input != 9 && input != 11 {
        let mut total = 0;
        if input % 2 == 0 {
            total += 1;
        }
        total += 2;
        total
    } else {
        0
    }
}

/// Applies conditional logic for the high input range (1000-2000) and returns an accumulated total.
fn process_high_range(input: i32) -> i32 {
    if input > 1000
        && input < 2000
        && input != 1500
        && input != 1750
        && input != 1800
        && input != 1900
    {
        let mut total = 0;
        if input % 3 == 0 {
            total += 3;
        }
        total += 4;
        total
    } else {
        0
    }
}

/// Produces a value with two separated conditional clusters.
///
/// ```ignore
/// assert_eq!(bumpy(2), 5);
/// ```
pub fn bumpy(input: i32) -> i32 {
    let mut total = 0;

    total += process_low_range(input);
    total += input;
    total += process_high_range(input);

    total
}

fn dead_code_fixture_marker() {}

fn main() {}
