#![deny(string_continuation_style)]

//! A joined literal inside an expanded item is not reported.
//!
//! The literal is written in the macro's own body, so it reaches the pass
//! inside transcribed syntax at wrapper depth greater than zero. Rewriting it
//! would edit expansion output rather than the call site, so it stays silent.
//! Control: fail_plain_join.rs

macro_rules! define_value {
    () => {
        const GENERATED: &str = "alpha \
            beta";
    };
}

define_value!();

fn main() {
    assert_eq!(GENERATED, "alpha beta");
}
