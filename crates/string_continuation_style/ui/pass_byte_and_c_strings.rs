// edition:2024
#![deny(string_continuation_style)]

//! Cooked byte and C strings keep their continuations.
//!
//! `concat!()` yields a `&str`, so rewriting a byte string would change its
//! type, and no stable macro assembles a `CStr` from parts. Both literals are
//! required to keep their source-line joins, so this fixture must stay silent.
//! Control: fail_plain_join.rs

const BYTE_VALUE: &[u8] = b"alpha \
    beta";
const C_VALUE: &core::ffi::CStr = c"alpha \
    beta";

fn main() {
    assert_eq!(BYTE_VALUE, b"alpha beta");
    assert_eq!(C_VALUE.to_bytes(), b"alpha beta");
}
