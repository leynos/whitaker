// edition:2024
#![deny(string_continuation_style)]

const BYTE_VALUE: &[u8] = b"alpha \
    beta";
const C_VALUE: &core::ffi::CStr = c"alpha \
    beta";

fn main() {
    assert_eq!(BYTE_VALUE, b"alpha beta");
    assert_eq!(C_VALUE.to_bytes(), b"alpha beta");
}
