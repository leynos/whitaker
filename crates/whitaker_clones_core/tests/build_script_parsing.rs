//! Verifies the build script's parser dependency extraction rules.

#[path = "../build_support.rs"]
mod build_support;

use build_support::{exact_version, parser_dependency_requirement};
use proptest::prelude::*;
use rstest::rstest;

#[rstest]
#[case::inline("[workspace.dependencies]\nra_ap_syntax = \"=0.0.334\"\n", "=0.0.334")]
#[case::table(
    "[workspace.dependencies.ra_ap_syntax]\nversion = \"=0.0.334\"\noptional = true\n",
    "=0.0.334"
)]
fn extracts_supported_dependency_forms(#[case] manifest: &str, #[case] expected: &str) {
    assert_eq!(
        parser_dependency_requirement(manifest).expect("dependency requirement should parse"),
        expected
    );
}

#[rstest]
#[case::missing("[workspace.dependencies]\nserde = \"1\"\n", "is missing")]
#[case::missing_version(
    "[workspace.dependencies.ra_ap_syntax]\noptional = true\n",
    "has no version string"
)]
fn rejects_invalid_dependency_forms(#[case] manifest: &str, #[case] expected: &str) {
    let error = parser_dependency_requirement(manifest)
        .expect_err("invalid dependency form should be rejected");

    assert!(error.to_string().contains(expected));
}

#[rstest]
#[case::caret("0.0.334")]
#[case::caret_explicit("^0.0.334")]
#[case::empty_exact("=")]
fn rejects_non_exact_requirements(#[case] requirement: &str) {
    assert!(exact_version(requirement).is_err());
}

#[rstest]
fn accepts_exact_requirement() {
    assert_eq!(
        exact_version("=0.0.334").expect("exact requirement should parse"),
        "0.0.334"
    );
}

proptest! {
    #[test]
    fn exact_version_accepts_only_non_empty_exact_pins(
        prefix in prop_oneof![Just("=".to_owned()), Just("^".to_owned()), Just("".to_owned())],
        suffix in "[A-Za-z0-9._-]{0,32}",
    ) {
        let requirement = format!("{prefix}{suffix}");
        let is_exact_pin = prefix == "=" && !suffix.is_empty();

        prop_assert_eq!(exact_version(&requirement).is_ok(), is_exact_pin);
    }
}
