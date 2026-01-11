//! UI test: panicking `unwrap_or_else` should be denied inside #[test].
#![deny(no_unwrap_or_else_panic)]

#[test]
fn flags_panicking_fallbacks_in_tests() {
    let value: Result<(), &str> = Err("boom");
    let _ = value.unwrap_or_else(|err| panic!("{}", err));
}

fn main() {}
