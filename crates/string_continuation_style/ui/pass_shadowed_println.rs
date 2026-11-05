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
//! silent here. The paired control removes the shadow detector, after which
//! this same file reports a diagnostic; that transcript is recorded in the
//! ExecPlan rather than kept as a fixture.

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
