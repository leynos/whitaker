//! Compile-time UI smoke tests for the `rstest_helper_should_be_fixture`
//! bootstrap crate.
//!
//! These trybuild cases intentionally expect no diagnostics while the lint is
//! in its registration and configuration-loading phase.

#[test]
fn bootstrap_fixtures_compile_without_diagnostics() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/bootstrap_zero_diagnostic.rs");
}
