//! Compile-time contracts for the public settings-normalization API.

#[test]
fn normalize_settings_is_public_and_the_legacy_name_stays_removed() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/api_ui/pass_normalize_settings.rs");
    cases.compile_fail("tests/api_ui/legacy_normalize_settings.rs");
}
