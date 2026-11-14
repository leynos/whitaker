//! Guard placement fixture: a guard between two returns keeps them apart.
//!
//! The guard sits on the second arm on purpose. If guards were recorded before
//! the arms were walked, all three returns would become adjacent in the event
//! stream and the score would be three, one over the configured limit. Walking
//! the arms in order places the guard and its comparison between the first and
//! second returns, so both returns charge and the score is two.

pub mod fixture {
    //! One match whose guard separates returns written in later arms.

    // 2 = baseline + the authored three-arm match + the authored guard
    // - floor(2 authored returns / 2).
    pub fn guarded_returns(value: Option<u8>) -> u8 {
        match value {
            None => return 0,
            Some(candidate) if candidate > 0 => return 1,
            Some(_) => return 2,
        }
    }
}

// 1: below the limit.
fn main() {}
