//! Compile-time tests for the fixture-expansion lint attribute.

#[test]
fn fixture_expansion_lint_attribute_contract() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/fixture_expansion_lint.rs");
    tests.compile_fail("tests/ui/fixture_expansion_lint_arguments.rs");
}
