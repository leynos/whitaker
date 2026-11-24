//! Boundary fixtures with the limit set to two.

pub mod fixture {
    //! Functions sitting exactly at, and one over, the configured limit.

    // 2 = baseline + authored `if`: equal to the limit, so it stays silent.
    pub fn at_limit(flag: bool) { if flag {} }

    // 3 = baseline + the desugared two-arm match + the authored `if`: one over
    // the limit.
    pub fn above_limit(values: &[u8]) { for value in values { if *value > 0 {} } }
}

// 1: below the limit.
fn main() {}