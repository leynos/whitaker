//! UI test: lint triggers inside `main` unless configured otherwise.
#![deny(no_unwrap_or_else_panic)]

fn main() {
    let value: Result<u8, &str> = Err("oops");
    let _ = value.unwrap_or_else(|err| panic!("{err}", err = err));
}
