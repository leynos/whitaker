//! Compile-time contracts for the externally exposed SARIF spelling changes.

/// Verify that renamed SARIF APIs compile and legacy names remain removed.
#[test]
fn renamed_sarif_api_is_public_and_legacy_names_stay_removed() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass_renamed_sarif_api.rs");
    cases.compile_fail("tests/ui/legacy_sarif_api.rs");
    cases.compile_fail("tests/ui/legacy_sarif_fields.rs");
}
