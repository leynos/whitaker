// edition:2024
//! A macro that duplicates its argument charges the author's code once.
//!
//! `tracing` with its `log` feature expands a call into two copies of every
//! argument, and each copy carries the one source span the author wrote. The
//! configured limit is zero, so every scored function warns, and each comment
//! records the hand-derived score.

pub mod fixture {
    //! Macros that put an authored argument into their expansion twice.

    macro_rules! twice {
        ($value:expr) => {{
            let _first = $value;
            let _second = $value;
        }};
    }

    /// Charge the duplicated `if` once.
    ///
    /// Scores 2: the baseline plus the authored `if`, though it expands twice.
    pub fn duplicated_if(flag: bool) { twice!(if flag { 1 } else { 2 }); }

    /// Score the control for `duplicated_if`.
    ///
    /// Scores 2: the same `if` written once outside a macro.
    pub fn single_if(flag: bool) { let _value = if flag { 1 } else { 2 }; }

    /// Charge the duplicated match and its guard once.
    ///
    /// Scores 3: the baseline plus the authored two-arm match plus its
    /// authored guard, each charged once.
    pub fn duplicated_match_guard(value: Option<u8>) {
        twice!(match value { Some(v) if v > 0 => v, _ => 0 });
    }

    /// Charge the duplicated `for` once.
    ///
    /// Scores 2: the baseline plus the authored `for`.
    pub fn duplicated_for(values: &[u8]) { twice!(for _value in values {}); }

    /// Keep a duplicated `return` adjacent to the `return` of the other arm.
    ///
    /// Scores 2: the baseline plus the authored two-arm match. The duplicated
    /// `return` is one return, so the two count once, which halves away outside
    /// a `Result` body.
    pub fn duplicated_return(value: u8) {
        match value { 0 => twice!(return), _ => return }
    }

    /// Drop a duplicated `if` that holds returns without changing adjacency.
    ///
    /// Scores 2, the same as `if_with_returns_control`: the whole second copy
    /// leaves the stream, so the `return` that follows is as adjacent as in
    /// the macro-free form.
    pub fn duplicated_if_with_returns(value: u8, flag: bool) {
        match value {
            0 => return,
            1 => twice!(if flag { return } else { return }),
            _ => return,
        }
    }

    /// Score the control for `duplicated_if_with_returns`.
    ///
    /// Scores 2: three branches (the baseline, the match and the `if`) minus
    /// half of three counted returns, truncated.
    pub fn if_with_returns_control(value: u8, flag: bool) {
        match value {
            0 => return,
            1 => if flag { return } else { return },
            _ => return,
        }
    }

    /// Charge two different call sites separately.
    ///
    /// Scores 3: the baseline plus two different authored `if`s, one per call.
    /// Only a repeat of the same source span is skipped, not a second call
    /// site.
    pub fn two_call_sites(flag: bool, other: bool) {
        twice!(if flag { 1 } else { 2 });
        twice!(if other { 1 } else { 2 });
    }
}

/// Satisfy the harness with an empty entry point.
fn main() {}
