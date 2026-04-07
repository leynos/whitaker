//! Regression example covering `#[tokio::test]` inside a module with a
//! non-standard test name, compiled under `--test` harness.
//!
//! This validates that the `has_test_module_name` fallback correctly recognises
//! test functions inside modules whose names follow test naming conventions
//! (e.g. `service_tests`) even when `#[cfg(test)]` is not present in HIR.
//!
//! The module is inline rather than `#[path]`-loaded because `Test::example()`
//! copies only the single `.rs` file to a temp directory and does not preserve
//! subdirectories.
//!
//! Regression test for <https://github.com/leynos/whitaker/issues/132>.

mod service_tests {
    #[tokio::test]
    async fn expect_in_path_module_is_allowed() {
        let value: Result<&str, ()> = Ok("ok");
        value.expect("non-standard module name test should permit expect");
    }
}

fn main() {}
