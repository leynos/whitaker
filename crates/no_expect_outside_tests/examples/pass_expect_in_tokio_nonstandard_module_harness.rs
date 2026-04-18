//! Regression example covering `#[tokio::test]` inside a non-standard module.
//!
//! A real Tokio proc macro under `rustc --test` strips the source-level test
//! attribute from the HIR function item. This example keeps the test in a
//! module named `service_tests` so the lint must rely on harness detection
//! rather than the conventional `mod tests` fallback.

#[cfg(test)]
mod service_tests {
    #[tokio::test]
    async fn pass_expect_in_tokio_nonstandard_module_harness() {
        let value = Ok::<_, ()>("ok");
        value.expect("Tokio tests in non-standard modules should permit expect");
    }
}

fn main() {}
