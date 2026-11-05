#![deny(string_continuation_style)]

//! A crate containing a glob import disables the lint for the crate.
//!
//! `use helpers::*;` binds whatever `helpers` exports, and this pass has no
//! name resolution to ask what that is. If it exports a `concat`, the suggested
//! bare `concat!(...)` is ambiguous and rustc rejects the rewrite with E0659 at
//! the very call site the lint just rewrote. Silence is the only sound answer
//! available, so a source-written glob counts as shadowing.
//!
//! The joined literal below would be flagged in a crate without a glob. It must
//! stay silent here.
//!
//! This fixture is the end-to-end proof that the glob arm is live. The compiler
//! injects `use std::prelude::rust_20xx::*;` into every crate, so an arm that
//! treated every glob as shadowing would silence the lint everywhere and every
//! `fail_*` fixture would stop reporting. The detector tests that span instead:
//! the injected prelude carries `DUMMY_SP`, a source glob does not.

mod helpers {
    pub fn reach() -> usize { 1 }
}

use helpers::*;

fn main() {
    let value: &str = "alpha \
        beta";
    println!("{}", value.len() + reach());
}
