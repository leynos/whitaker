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
//! 1. **Technical constraint**: The `dylint_testing` harness uses `CARGO_PKG_NAME` at compile time,
//!    preventing fixture crates from having controlled names.
//!
//! 2. **Unit test coverage is sufficient**: The exclusion implementation is straightforward—when
//!    `self.excluded` is true, `emit_optional` returns early (see `driver.rs`). All configuration
//!    parsing, deserialization, and matching logic is fully validated by the tests below.
//!
//! 3. **Behaviour is deterministic**: The `check_crate` method sets `self.excluded` based on
//!    `config.is_excluded(crate_name)`, which is exhaustively tested here.
//!
//! The integration tests in `tests/integration_exclusion.rs` provide additional
//! coverage by invoking `cargo dylint` on fixture projects with real exclusion
//! configurations.

use std::{collections::HashSet, io};

use rstest::rstest;

use super::*;

/// Check that the embedded revision matches the build environment.
///
/// A build that does not set `WHITAKER_SUITE_REVISION` reports `"unknown"`
/// rather than inventing provenance.
#[test]
fn suite_revision_reports_the_build_environment() {
    let expected = option_env!("WHITAKER_SUITE_REVISION").unwrap_or("unknown");
    assert_eq!(suite_revision(), expected);
}

/// Build a boundary view for mock-reader and loader tests.
fn boundary(
    excluded_crates: &[&str],
    excluded_paths: &[&str],
    unknown_keys: &[&str],
) -> BoundaryConfig {
    BoundaryConfig {
        excluded_crates: excluded_crates
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        excluded_paths: excluded_paths
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        unknown_keys: unknown_keys
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

#[test]
fn config_default_has_empty_excluded_crates() {
    assert!(NoStdFsConfig::default().excluded_crates.is_empty());
}

#[test]
fn config_default_has_empty_excluded_paths() {
    assert!(NoStdFsConfig::default().excluded_paths.is_empty());
}

#[rstest]
#[case::empty_config(r"", &[])]
#[case::empty_excluded(r"excluded_crates = []", &[])]
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
#[case::empty_config(r"", &[])]
#[case::empty_excluded(r"excluded_paths = []", &[])]
#[case::single_path(r#"excluded_paths = ["my_app::legacy_io"]"#, &["my_app::legacy_io"])]
#[case::multiple_paths(
    r#"excluded_paths = ["my_app::legacy_io", "my_app::bin::migrate"]"#,
    &["my_app::legacy_io", "my_app::bin::migrate"]
)]
fn config_deserializes_excluded_paths(#[case] toml: &str, #[case] expected: &[&str]) {
    let config: NoStdFsConfig = toml::from_str(toml).expect("valid TOML");
    assert_eq!(
        config.excluded_paths,
        expected
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<HashSet<_>>()
    );
}

#[test]
fn config_deserializes_both_exclusion_kinds_together() {
    // A configuration mixing both fields must parse without either field
    // shadowing the other.
    let toml = r#"
        excluded_crates = ["helper_crate"]
        excluded_paths = ["my_app::legacy_io"]
    "#;
    let config: NoStdFsConfig = toml::from_str(toml).expect("valid TOML");
    assert_eq!(
        config.excluded_crates,
        HashSet::from(["helper_crate".to_owned()])
    );
    assert_eq!(
        config.excluded_paths,
        HashSet::from(["my_app::legacy_io".to_owned()])
    );
}

#[test]
fn legacy_config_without_excluded_paths_still_parses() {
    // Backward compatibility: existing configs that predate `excluded_paths`
    // must continue to parse, leaving the new field empty.
    let config: NoStdFsConfig = toml::from_str(r#"excluded_crates = ["foo"]"#).expect("valid TOML");
    assert_eq!(config.excluded_crates, HashSet::from(["foo".to_owned()]));
    assert!(config.excluded_paths.is_empty());
}

#[rstest]
#[case::wrong_type(r#"excluded_paths = "not_an_array""#)]
#[case::wrong_element_type(r"excluded_paths = [1, 2, 3]")]
#[case::mixed_element_types(r#"excluded_paths = ["my_app::legacy_io", 1]"#)]
fn config_rejects_invalid_excluded_paths(#[case] toml: &str) {
    assert!(
        toml::from_str::<NoStdFsConfig>(toml).is_err(),
        "expected error for: {toml}"
    );
}

#[rstest]
#[case::within_excluded_module(&["my_app::legacy_io"], "my_app::legacy_io::reader", true)]
#[case::exact_excluded_module(&["my_app::legacy_io"], "my_app::legacy_io", true)]
#[case::sibling_not_excluded(&["my_app::legacy_io"], "my_app::legacy_io_utils", false)]
#[case::unrelated_not_excluded(&["my_app::legacy_io"], "my_app::network", false)]
#[case::no_paths_configured(&[], "my_app::legacy_io", false)]
fn path_exclusions_reflect_configuration(
    #[case] configured: &[&str],
    #[case] item: &str,
    #[case] expected: bool,
) {
    let config = NoStdFsConfig {
        excluded_crates: HashSet::new(),
        excluded_paths: configured.iter().map(|s| (*s).to_owned()).collect(),
    };
    let exclusions = config.path_exclusions();
    assert_eq!(
        exclusions.excludes(&whitaker_common::SimplePath::parse(item)),
        expected
    );
}

#[rstest]
#[case::unknown_field(r"unknown_field = true")]
#[case::wrong_type(r#"excluded_crates = "not_an_array""#)]
#[case::wrong_element_type(r"excluded_crates = [1, 2, 3]")]
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
#[case::hyphenated_package_name_does_not_match_crate_identifier(
    &["whitaker-common"],
    "whitaker_common",
    false
)]
#[case::underscored_crate_identifier_matches_runtime_name(
    &["whitaker_common"],
    "whitaker_common",
    true
)]
fn is_excluded_matches_correctly(
    #[case] excluded: &[&str],
    #[case] query: &str,
    #[case] expected: bool,
) {
    let config = NoStdFsConfig {
        excluded_crates: excluded.iter().map(|s| (*s).to_owned()).collect(),
        excluded_paths: HashSet::new(),
    };
    assert_eq!(config.is_excluded(query), expected);
}

#[rstest]
#[case::scalar(r"future_option = true", &["future_option"])]
#[case::quoted_key(r#""future option" = true"#, &["future option"])]
#[case::nested_table("[future_table]\nenabled = true", &["future_table"])]
#[case::local_date("future_date = 2026-01-02", &["future_date"])]
#[case::local_time("future_time = 07:32:00", &["future_time"])]
fn boundary_config_keeps_known_keys_and_records_unknown_values(
    #[case] unknown_toml: &str,
    #[case] expected_unknown: &[&str],
) {
    let toml = format!(
        concat!(
            "excluded_crates = [\"helper_crate\"]\n",
            "excluded_paths = [\"my_app::legacy_io\"]\n",
            "{unknown_toml}\n",
        ),
        unknown_toml = unknown_toml,
    );
    let boundary: BoundaryConfig = toml::from_str(&toml).expect("boundary config should parse");
    let (config, unknown_keys) = boundary.into_parts();

    assert_eq!(
        config.excluded_crates,
        HashSet::from(["helper_crate".to_owned()])
    );
    assert_eq!(
        config.excluded_paths,
        HashSet::from(["my_app::legacy_io".to_owned()])
    );
    assert_eq!(unknown_keys, expected_unknown);
}

#[test]
fn boundary_config_sorts_unknown_key_names() {
    let toml = concat!(
        "zulu = 1\n",
        "alpha = 2\n",
        "[middle]\n",
        "enabled = true\n",
    );
    let boundary: BoundaryConfig = toml::from_str(toml).expect("boundary config should parse");
    let (_, unknown_keys) = boundary.into_parts();
    assert_eq!(unknown_keys, ["alpha", "middle", "zulu"]);
}

#[test]
fn boundary_config_defaults_missing_fields() {
    let boundary: BoundaryConfig = toml::from_str("").expect("empty boundary config should parse");
    let (config, unknown_keys) = boundary.into_parts();
    assert!(config.excluded_crates.is_empty());
    assert!(config.excluded_paths.is_empty());
    assert!(unknown_keys.is_empty());
}

#[rstest]
#[case::wrong_crates_type(r#"excluded_crates = "not_an_array""#)]
#[case::wrong_crate_element_type("excluded_crates = [1, 2, 3]")]
#[case::wrong_paths_type(r#"excluded_paths = "not_an_array""#)]
#[case::wrong_path_element_type(r#"excluded_paths = ["my_app::legacy_io", 1]"#)]
fn boundary_config_rejects_wrong_known_types(#[case] toml: &str) {
    assert!(
        toml::from_str::<BoundaryConfig>(toml).is_err(),
        "expected error for: {toml}"
    );
}

#[test]
fn load_configuration_returns_config_when_present() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config()
        .returning(|_| Ok(Some(boundary(&["my_crate"], &["my_app::io"], &[]))));

    let load = load_configuration_with_reader(&mock);
    assert_eq!(
        load.config.excluded_crates,
        HashSet::from(["my_crate".to_owned()])
    );
    assert_eq!(
        load.config.excluded_paths,
        HashSet::from(["my_app::io".to_owned()])
    );
    assert!(load.issues.is_empty());
}

#[test]
fn load_configuration_keeps_known_exclusions_when_keys_are_unknown() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config().returning(|_| {
        Ok(Some(boundary(
            &["helper_crate"],
            &["my_app::legacy_io"],
            &["future_option"],
        )))
    });

    let load = load_configuration_with_reader(&mock);
    assert_eq!(
        load.config.excluded_crates,
        HashSet::from(["helper_crate".to_owned()])
    );
    assert_eq!(
        load.config.excluded_paths,
        HashSet::from(["my_app::legacy_io".to_owned()])
    );
    assert_eq!(
        load.issues,
        [ConfigIssue::UnknownKeys(vec!["future_option".to_owned()])]
    );
}

#[test]
fn load_configuration_returns_default_when_none() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config().returning(|_| Ok(None));

    let load = load_configuration_with_reader(&mock);
    assert!(load.config.excluded_crates.is_empty());
    assert!(load.config.excluded_paths.is_empty());
    assert!(load.issues.is_empty());
}

#[test]
fn load_configuration_returns_default_on_error() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config().returning(|_| {
        Err(Box::new(io::Error::other("parse error"))
            as Box<dyn std::error::Error + Send + Sync + 'static>)
    });

    let load = load_configuration_with_reader(&mock);
    assert!(load.config.excluded_crates.is_empty());
    assert_eq!(
        load.issues,
        [ConfigIssue::SectionDiscarded("parse error".to_owned())]
    );
}
