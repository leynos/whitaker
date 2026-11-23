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

    // 2 = baseline + the authored `if` charged once, though it expands twice.
    pub fn duplicated_if(flag: bool) { twice!(if flag { 1 } else { 2 }); }

    // 2: the control, with the same `if` written once outside a macro.
    pub fn single_if(flag: bool) { let _value = if flag { 1 } else { 2 }; }

    // 3 = baseline + the authored two-arm match + its authored guard, each
    // charged once.
    pub fn duplicated_match_guard(value: Option<u8>) {
        twice!(match value { Some(v) if v > 0 => v, _ => 0 });
    }

    // 2 = baseline + the authored `for`, charged once.
    pub fn duplicated_for(values: &[u8]) { twice!(for _value in values {}); }

    // 2 = baseline + the authored two-arm match: the duplicated `return` is one
    // return, so it stays adjacent to the `return` of the other arm and the two
    // count once, which halves away outside a `Result` body.
    pub fn duplicated_return(value: u8) {
        match value { 0 => twice!(return), _ => return }
    }

    // 3 = baseline + two different authored `if`s, one per call. Only a repeat
    // of the same source span is skipped, not a second call site.
    pub fn two_call_sites(flag: bool, other: bool) {
        twice!(if flag { 1 } else { 2 });
        twice!(if other { 1 } else { 2 });
    }
}

fn main() {}
