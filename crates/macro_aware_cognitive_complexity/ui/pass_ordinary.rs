//! Ordinary code that stays far below the default limit of twenty-five.

// 3 = baseline + the authored three-arm match + its authored guard.
fn classify(value: Option<u8>) -> u8 {
    match value {
        Some(candidate) if candidate > 10 => 2,
        Some(candidate) => candidate,
        None => 0,
    }
}

// 1: below the default limit.
fn main() { let _ = classify(Some(3)); }