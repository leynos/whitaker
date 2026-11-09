// edition:2024
//! Threshold-zero fixtures for the macro-aware complexity lint.
//!
//! The configured limit is zero, so every function the lint scores warns. Each
//! comment records the hand-derived score from the policy arithmetic: a
//! baseline of one, plus one per authored branch or guard, minus the return
//! adjustment.

pub mod fixture {
    //! Authored and generated constructs the lint must tell apart.

    // 1: the baseline.
    pub fn baseline() {}

    // 2 = baseline + authored `if`.
    pub fn authored_if(flag: bool) { if flag {} }

    // 1: the two-arm match comes from the `matches!` macro and is generated.
    pub fn generated_match(value: Option<u8>) { let _ = matches!(value, Some(_)); }

    // 2 = baseline + the authored `if` passed as a macro argument; the generated
    // `if` inside `assert!` is not charged.
    pub fn authored_if_in_assert(flag: bool) { assert!(if flag { true } else { false }); }

    // 2 = baseline + the authored `if` passed as a macro argument; the
    // `println!` expansion is generated.
    pub fn authored_if_in_println(flag: bool) { println!("{}", if flag { 1 } else { 2 }); }

    // 2 = baseline + the authored guard; the generated two-arm match is excluded.
    pub fn authored_guard(value: Option<u8>) { let _ = matches!(value, Some(v) if v > 0); }

    // 1: the generated `if` inside `assert!` is not charged.
    pub fn logical_assert(a: bool, b: bool) { assert!(a && b); }

    // 3 = baseline + the desugared two-arm match + the authored `if`. Desugaring
    // marks are transparent, so both remain authored.
    pub fn source_for(values: &[u8]) { for value in values { if *value > 0 {} } }

    // 2 = baseline + the desugared `if` of a while loop; the desugaring is
    // transparent.
    pub fn source_while(mut count: u8) { while count > 0 { count -= 1; } }

    // 1 = two branches minus one full return credit: the `?` lowering
    // contributes a two-arm match and one return, and a `Result` body credits
    // returns in full.
    pub fn result_try(input: Result<u8, ()>) -> Result<u8, ()> { let value = input?; Ok(value) }

    // 2 = baseline + the authored `if`; the coroutine wrapper is not scored.
    pub async fn async_if(flag: bool) { if flag {} }

    // 1: the async-block wrapper is a generated coroutine closure, so its body
    // is deliberately unscored; this matches pinned Clippy's blind spot.
    pub fn async_block(flag: bool) { let future = async { if flag {} }; let _ = future; }

    // Outer 1; the authored closure anchors on its header and scores 2.
    pub fn with_closure() { let callback = |flag: bool| if flag {}; let _ = callback; }

    macro_rules! call {
        ($f:expr) => {
            $f(true)
        };
    }

    // Caller 1; the authored closure passed as a macro argument still anchors
    // on its header and scores 2.
    pub fn closure_argument() { call!(|flag: bool| if flag {}); }

    macro_rules! body_if {
        () => {
            if true {}
        };
    }

    // 1: the expanded `if` carries a macro mark and is generated.
    pub fn generated_body() { body_if!(); }

    macro_rules! identity {
        ($expr:expr) => {
            $expr
        };
    }

    // 2 = baseline + the authored argument expression; the macro adds nothing.
    pub fn authored_argument(flag: bool) { identity!(if flag {}); }

    macro_rules! when {
        ($condition:expr, $body:block) => {
            if $condition $body
        };
    }

    // 2: the expanded outer `if` is generated; the authored body `if` is
    // charged.
    pub fn mixed(flag: bool) { when!(flag, { if flag {} }); }

    macro_rules! wrapped {
        ($body:block) => {
            (|| $body)()
        };
    }

    // 1: the generated closure is skipped, so its authored `if` is not scored.
    pub fn generated_closure(flag: bool) { wrapped!({ if flag {} }); }

    // Outer 1; the nested item is visited separately and scores 2.
    pub fn outer() {
        fn inner(flag: bool) { if flag {} }
        inner(false);
    }

    // No diagnostic: functions carrying `#[test]` are skipped.
    #[test]
    pub fn test_shaped(flag: bool) { if flag {} }

    pub struct Service;

    impl Service {
        // 2 = baseline + authored `if`; anchored on the method identifier.
        pub fn method(&self, flag: bool) { if flag {} }
    }

    // 2 = baseline + the desugared two-arm poll match of `.await`. The packet
    // listed one here, but the lowering charges the match; pinned Clippy agrees.
    pub async fn awaited() { std::future::ready(()).await; }

    macro_rules! in_range {
        ($value:expr) => {
            matches!($value, 1..=9)
        };
    }

    // 3: baseline + the authored two-arm match + the authored guard expression
    // `v > 0 && in_range!(v)`. The generated `matches!` inside the guard is
    // excluded, but the guard's own authored span is still charged.
    pub fn authored_match_generated_guard(value: Option<u8>) -> u8 {
        match value {
            Some(v) if v > 0 && in_range!(v) => v,
            _ => 0,
        }
    }

    // ---- Remaining V4 real-compiler cases --------------------------------

    // 2 = baseline + the authored `if`; the `matches!` match inside it is
    // generated.
    pub fn authored_if_wrapping_generated_match(value: Option<u8>, flag: bool) {
        if flag { let _ = matches!(value, Some(_)); }
    }

    // 1: `assert!` and `matches!` both expand to generated branches.
    pub fn nested_generated_branches(a: bool, b: bool) { assert!(matches!(a && b, true)); }

    macro_rules! guarded_match_body {
        ($value:expr) => {
            match $value { Some(v) if v > 0 => v, _ => 0 }
        };
    }

    // 1: the two-arm match and its guard are written in a macro body.
    pub fn generated_guard_ignored(value: Option<u8>) -> u8 { guarded_match_body!(value) }

    pub struct Wrapper(u8);

    // 1: a single-arm match is not a branch.
    pub fn authored_single_arm_match(value: Wrapper) -> u8 { match value { Wrapper(v) => v } }

    // 2 = baseline + the authored two-arm match.
    pub fn authored_two_arm_match(value: Option<u8>) -> u8 {
        match value { Some(v) => v, None => 0 }
    }

    macro_rules! body_for {
        ($values:expr) => {
            for value in $values { let _ = *value; }
        };
    }

    // 1: the desugared `for` match is generated inside the macro body.
    pub fn generated_body_for(values: &[u8]) { body_for!(values) }

    macro_rules! body_while {
        ($count:expr) => {{
            let mut count = $count;
            while count > 0 { count -= 1; }
        }};
    }

    // 1: the desugared `while` if is generated inside the macro body.
    pub fn generated_body_while(count: u8) { body_while!(count) }

    macro_rules! body_try {
        ($input:expr) => {{
            let value = $input?;
            Ok(value)
        }};
    }

    // 1: the `?` lowering's match and return are generated; a Result body
    // credits only authored returns.
    pub fn generated_body_try(input: Result<u8, ()>) -> Result<u8, ()> { body_try!(input) }

    // 1 = baseline + the authored `?` match minus the authored return: the
    // argument is authored, so its desugaring is transparent.
    pub fn authored_argument_try(input: Result<u8, ()>) -> Result<u8, ()> {
        identity!(Ok(input?))
    }

    macro_rules! body_await {
        ($future:expr) => {
            $future.await;
        };
    }

    // 1: the `await` poll match is generated inside the macro body.
    pub async fn generated_body_await() { body_await!(std::future::ready(())); }

    // 2 = baseline + the authored poll match of the argument's `.await`.
    pub async fn authored_argument_await(future: impl core::future::Future<Output = u8>) -> u8 {
        identity!(future.await)
    }

    macro_rules! body_async_block {
        () => {
            async { if true {} }
        };
    }

    // 1: the generated async-block wrapper is skipped, so its body is unscored.
    pub fn generated_body_async_block() {
        let future = body_async_block!();
        let _ = future;
    }

    // 1: an authored async block is still skipped by the coroutine-wrapper
    // rule; pinned Clippy shares this blind spot.
    pub fn authored_argument_async_block(flag: bool) {
        let future = identity!(async { if flag {} });
        let _ = future;
    }

    macro_rules! generated_async_item {
        () => {
            async fn generated_async(flag: bool) { if flag {} }
        };
    }

    // No diagnostic: the item's identifier itself comes from a macro body.
    generated_async_item!();

    // 2 = 3 branches - floor(2 authored returns / 2): a non-Result body halves
    // the return credit.
    pub fn non_result_returns(flag: bool, other: bool) -> u8 {
        if flag { return 1; }
        if other { return 2; }
        3
    }

    // 1 = 3 branches - 2: a Result body credits both authored returns in full.
    pub fn result_returns(flag: bool, other: bool) -> Result<u8, ()> {
        if flag { return Ok(1); }
        if other { return Ok(2); }
        Ok(3)
    }

    macro_rules! early_return {
        () => {
            return 0;
        };
    }

    // 2 = baseline + the authored `if`: the generated return is not charged and
    // does not credit the non-Result adjustment.
    pub fn generated_return(flag: bool) -> u8 {
        if flag { early_return!(); }
        1
    }

    // 4 = baseline + the authored `if` + the authored two-arm match + the
    // authored guard; the generated `matches!` inside the guard is excluded.
    pub fn mixed_authored_if_match_guard(value: Option<u8>, flag: bool) -> u8 {
        if flag { return 0; }
        match value {
            Some(v) if v > 0 && in_range!(v) => v,
            _ => 0,
        }
    }
}

// 1: the empty entry point behaves like any other baseline function.
fn main() {}