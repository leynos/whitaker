//! UI test: a plain helper inside `#[cfg(test)] mod tests` is test-only code.
//!
//! Every other `cfg(test)` fixture here puts the call inside a `#[test]`
//! function, so it passes on the test attribute and says nothing about whether
//! module ancestry was detected. This helper carries no attribute, so it
//! passes only when the `cfg(test)` on its parent module is seen. The panic
//! interpolates, because this lint allows a panicking fallback in test code
//! only when the message carries a runtime value.
#![deny(no_unwrap_or_else_panic)]

#[cfg(test)]
mod tests {
    fn fallback(value: Result<i32, &str>) -> i32 {
        value.unwrap_or_else(|error| panic!("fallback failed: {error}"))
    }

    #[test]
    fn uses_the_helper() {
        assert_eq!(fallback(Ok(1)), 1);
    }
}

fn main() {}
