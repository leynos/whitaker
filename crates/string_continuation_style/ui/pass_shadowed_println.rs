#![deny(string_continuation_style)]

//! A crate that defines its own `println!` disables the lint for the crate.
//!
//! The lint names built-in macros without a module qualifier, both when it
//! reasons about expression position and when it suggests `concat!`. A crate
//! that can substitute its own macro for one of those names makes every
//! bare-name report unsound, so condition 3 of the RFC's §Macro position
//! disables the lint wholesale before the walk begins.
//!
//! The joined literal below would be flagged in any other crate. It must stay
//! silent here. The committed control is `fail_plain_join.rs`: byte-identical
//! joined literal, no shadowing construct, and it must report. If the shadow
//! detector were inverted or unreachable, this fixture would start reporting
//! too, and `tests/support/silence_control_tests.rs` fails when any `pass_*`
//! fixture stops being silent.
//! Control: fail_plain_join.rs

macro_rules! println {
    ($($tokens:tt)*) => {{
        let _ = format_args!($($tokens)*);
    }};
}

fn main() {
    let value: &str = "alpha \
        beta";
    println!("{}", value.len());
}
