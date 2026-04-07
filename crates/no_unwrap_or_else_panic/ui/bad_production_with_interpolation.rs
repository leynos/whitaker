//! UI test: interpolating panic outside tests is still denied.
#![deny(no_unwrap_or_else_panic)]

fn demo(flag: bool) -> i32 {
    let value: Result<i32, &str> = if flag { Ok(1) } else { Err("boom") };
    value.unwrap_or_else(|e| panic!("failed: {e}"))
}

fn main() {
    let _ = demo(true);
}
