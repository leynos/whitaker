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

    macro_rules! closure_then_direct {
        ($value:expr) => {{
            let _later = || $value;
            let _now = $value;
        }};
    }

    macro_rules! direct_then_closure {
        ($value:expr) => {{
            let _now = $value;
            let _later = || $value;
        }};
    }

    /// Charge a `return` once when the first copy is in a generated closure.
    ///
    /// Scores 1, the same as `single_return_control`: the copy inside the
    /// closure and the direct copy are one authored `return`, so the walk
    /// skips the second although the two copies have different owners.
    pub fn closure_then_direct_copies(a: bool, b: bool, c: bool) -> Result<u8, ()> {
        if a { return Ok(1); }
        if b { return Ok(2); }
        if c {}
        closure_then_direct!(return Err::<u8, ()>(()));
        Ok(3)
    }

    /// Charge a `return` once when the first copy is outside the closure.
    ///
    /// Scores 1, the same as `single_return_control`.
    pub fn direct_then_closure_copies(a: bool, b: bool, c: bool) -> Result<u8, ()> {
        if a { return Ok(1); }
        if b { return Ok(2); }
        if c {}
        direct_then_closure!(return Err::<u8, ()>(()));
        Ok(3)
    }

    /// Score the control for the two copies above.
    ///
    /// Scores 1: four branches minus three counted `Result` returns.
    pub fn single_return_control(a: bool, b: bool, c: bool) -> Result<u8, ()> {
        if a { return Ok(1); }
        if b { return Ok(2); }
        if c {}
        let _now = return Err::<u8, ()>(());
        Ok(3)
    }

    macro_rules! guard_in_two_matches {
        ($guard:expr) => {{
            let _first = match 1u8 { n if $guard => n, _ => 0 };
            let _second = match 2u8 { n if $guard => n, _ => 0 };
        }};
    }

    /// Charge an authored guard once when two generated matches copy it.
    ///
    /// Scores 3: the baseline plus the authored `if` inside the guard, charged
    /// once although both generated matches carry a copy; the generated
    /// matches themselves are not counted.
    pub fn guard_copied_into_generated_matches(flag: bool, other: bool) {
        guard_in_two_matches!(if flag { other } else { !other });
    }

    macro_rules! return_guard_in_two_matches {
        ($guard:expr) => {{
            let _first = match 1u8 { n if $guard => n, _ => 0 };
            let _second = match 2u8 { n if $guard => n, _ => 0 };
        }};
    }

    macro_rules! return_guard_in_one_match {
        ($guard:expr) => {{
            let _first = match 1u8 { n if $guard => n, _ => 0 };
        }};
    }

    /// Keep return adjacency when two generated matches copy a guard that
    /// ends in a `return`.
    ///
    /// Scores the same as `return_guard_in_one_generated_match`. The guard's
    /// block is an authored expression, so a copy of it that stayed in the
    /// stream would separate the guard's `return` from the authored `return`
    /// that follows. The repeat copy leaves whole, so the two stay adjacent.
    #[expect(unreachable_code, reason = "the guard returns before the match ends")]
    pub fn return_guard_copied_into_generated_matches() {
        return_guard_in_two_matches!({ return });
        return
    }

    /// Score the control for the two generated matches above.
    ///
    /// Scores the same: the guard is expanded into one generated match only.
    #[expect(unreachable_code, reason = "the guard returns before the match ends")]
    pub fn return_guard_in_one_generated_match() {
        return_guard_in_one_match!({ return });
        return
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
