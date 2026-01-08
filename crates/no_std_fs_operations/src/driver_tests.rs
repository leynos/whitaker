//! Unit tests for configuration parsing, exclusion logic, and config loading.
//!
//! Provides layered coverage: config parsing, exclusion matching, and mock-based
//! config loading. Integration testing of exclusion during lint execution is not
//! feasible with `dylint_testing` as crate names cannot be controlled.

use super::*;
use rstest::rstest;
use std::io;

#[test]
fn config_default_has_empty_excluded_crates() {
    assert!(NoStdFsConfig::default().excluded_crates.is_empty());
}

#[rstest]
#[case::empty_config(r#""#, vec![])]
#[case::empty_excluded(r#"excluded_crates = []"#, vec![])]
#[case::single_crate(r#"excluded_crates = ["foo"]"#, vec!["foo"])]
#[case::multiple_crates(r#"excluded_crates = ["foo", "bar", "baz"]"#, vec!["foo", "bar", "baz"])]
fn config_deserializes_excluded_crates(#[case] toml: &str, #[case] expected: Vec<&str>) {
    let config: NoStdFsConfig = toml::from_str(toml).expect("valid TOML");
    assert_eq!(
        config.excluded_crates,
        expected.into_iter().map(String::from).collect::<Vec<_>>()
    );
}

#[rstest]
#[case::unknown_field(r#"unknown_field = true"#)]
#[case::wrong_type(r#"excluded_crates = "not_an_array""#)]
#[case::wrong_element_type(r#"excluded_crates = [1, 2, 3]"#)]
fn config_rejects_invalid_toml(#[case] toml: &str) {
    assert!(
        toml::from_str::<NoStdFsConfig>(toml).is_err(),
        "expected error for: {toml}"
    );
}

#[rstest]
#[case::exact_match(&["my_crate", "other"], "my_crate", true)]
#[case::other_match(&["my_crate", "other"], "other", true)]
#[case::not_found(&["my_crate", "other"], "unknown", false)]
#[case::case_sensitive_match(&["MyCrate"], "MyCrate", true)]
#[case::case_sensitive_lowercase(&["MyCrate"], "mycrate", false)]
#[case::case_sensitive_uppercase(&["MyCrate"], "MYCRATE", false)]
fn is_excluded_matches_correctly(
    #[case] excluded: &[&str],
    #[case] query: &str,
    #[case] expected: bool,
) {
    let config = NoStdFsConfig {
        excluded_crates: excluded.iter().map(|s| (*s).to_owned()).collect(),
    };
    assert_eq!(config.is_excluded(query), expected);
}

#[test]
fn load_configuration_returns_config_when_present() {
    let config = NoStdFsConfig {
        excluded_crates: vec!["my_crate".to_owned()],
    };
    let mut mock = MockConfigReader::new();
    mock.expect_read_config()
        .returning(move |_| Ok(Some(config.clone())));
    assert_eq!(
        load_configuration_with_reader(&mock).excluded_crates,
        vec!["my_crate"]
    );
}

#[test]
fn load_configuration_returns_default_when_none() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config().returning(|_| Ok(None));
    assert!(
        load_configuration_with_reader(&mock)
            .excluded_crates
            .is_empty()
    );
}

#[test]
fn load_configuration_returns_default_on_error() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config()
        .returning(|_| Err(Box::new(io::Error::other("parse error")) as _));
    assert!(
        load_configuration_with_reader(&mock)
            .excluded_crates
            .is_empty()
    );
}
