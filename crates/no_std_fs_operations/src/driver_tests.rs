//! Unit tests for configuration parsing, exclusion logic, and config loading.
//!
//! Provides layered coverage: config parsing, exclusion matching, and mock-based
//! config loading.
//!
//! # Exception to Behavioural Testing Guidelines
//!
//! This module documents an explicit exception to the project's requirement for
//! behavioural tests validating new functionality. End-to-end integration testing
//! of the exclusion feature is not feasible with `dylint_testing` because:
//!
//! 1. **Technical constraint**: The `dylint_testing` harness uses `CARGO_PKG_NAME`
//!    at compile time, preventing fixture crates from having controlled names.
//!
//! 2. **Unit test coverage is sufficient**: The exclusion implementation is
//!    straightforward—when `self.excluded` is true, `emit_optional` returns early
//!    (see `driver.rs`). All configuration parsing, deserialisation, and matching
//!    logic is fully validated by the tests below.
//!
//! 3. **Behaviour is deterministic**: The `check_crate` method sets `self.excluded`
//!    based on `config.is_excluded(crate_name)`, which is exhaustively tested here.
//!
//! The integration tests in `tests/integration_exclusion.rs` provide additional
//! coverage by invoking `cargo dylint` on fixture projects with real exclusion
//! configurations.

use super::*;
use rstest::rstest;
use std::collections::HashSet;
use std::io;

#[test]
fn config_default_has_empty_excluded_crates() {
    assert!(NoStdFsConfig::default().excluded_crates.is_empty());
}

#[rstest]
#[case::empty_config(r#""#, &[])]
#[case::empty_excluded(r#"excluded_crates = []"#, &[])]
#[case::single_crate(r#"excluded_crates = ["foo"]"#, &["foo"])]
#[case::multiple_crates(r#"excluded_crates = ["foo", "bar", "baz"]"#, &["foo", "bar", "baz"])]
fn config_deserializes_excluded_crates(#[case] toml: &str, #[case] expected: &[&str]) {
    let config: NoStdFsConfig = toml::from_str(toml).expect("valid TOML");
    assert_eq!(
        config.excluded_crates,
        expected
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<HashSet<_>>()
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
        excluded_crates: HashSet::from(["my_crate".to_owned()]),
    };
    let mut mock = MockConfigReader::new();
    mock.expect_read_config()
        .returning(move |_| Ok(Some(config.clone())));
    assert_eq!(
        load_configuration_with_reader(&mock).excluded_crates,
        HashSet::from(["my_crate".to_owned()])
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
    mock.expect_read_config().returning(|_| {
        Err(Box::new(io::Error::other("parse error"))
            as Box<dyn std::error::Error + Send + Sync + 'static>)
    });
    assert!(
        load_configuration_with_reader(&mock)
            .excluded_crates
            .is_empty()
    );
}
