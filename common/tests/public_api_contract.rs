//! Compile-time contracts for common public APIs changed to Oxford spelling.

/// Verify that renamed common APIs compile and their legacy names remain removed.
#[test]
fn renamed_common_apis_are_public_and_legacy_names_stay_removed() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass_renamed_common_apis.rs");
    cases.compile_fail("tests/ui/legacy_common_apis.rs");
}
