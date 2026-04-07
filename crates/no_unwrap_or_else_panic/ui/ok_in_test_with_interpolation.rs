//! UI test: interpolating panic inside `#[test]` is permitted because the
//! closure incorporates a runtime value into the panic message.
#![deny(no_unwrap_or_else_panic)]

#[test]
fn interpolated_panic_is_allowed_in_test() {
    let value: Result<(), &str> = Err("boom");
    let _ = value.unwrap_or_else(|e| panic!("got: {e}"));
}

fn main() {}
