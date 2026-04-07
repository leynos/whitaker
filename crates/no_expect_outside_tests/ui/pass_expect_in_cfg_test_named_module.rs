// aux-build: tokio.rs
//! Positive UI fixture: allow `.expect(...)` in `#[tokio::test]` functions
//! within a `#[cfg(test)]` module whose name is not `test` or `tests`.
//!
//! Regression test for <https://github.com/leynos/whitaker/issues/132>.
#![deny(no_expect_outside_tests)]

extern crate core;
extern crate tokio;

#[cfg(test)]
mod check_tests {
    #[tokio::test]
    fn tokio_expect_in_named_module() {
        let option = Some("ok");
        option.expect("cfg(test) module with tokio::test permits expect");
    }
}

fn main() {}
