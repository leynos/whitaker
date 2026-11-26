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

/// Report an embedded revision verbatim, and `"unknown"` when there is none.
///
/// `option_env!` is fixed at compile time, so a test that re-read it could only
/// restate the implementation. Feeding the mapping both inputs is what proves
/// the fallback, and it fails if either branch returns a constant.
#[rstest]
#[case::embedded(Some("abc1234"), "abc1234")]
#[case::embedded_dirty(Some("abc1234+dirty"), "abc1234+dirty")]
#[case::absent(None, "unknown")]
fn suite_revision_maps_the_embedded_value(#[case] embedded: Option<&str>, #[case] expected: &str) {
    assert_eq!(
        suite_revision_from(embedded),
        expected,
        "the revision mapping should report {expected:?} for {embedded:?}"
    );
}

/// The exclusions a mock reader hands the loader, with no unknown keys.
///
/// The loader tests only care which exclusions survive, so this names one
/// representative configuration rather than parameterizing three string lists
/// that every caller would pass the same way.
fn excluded_configuration() -> BoundaryConfig {
    BoundaryConfig {
        excluded_crates: ["my_crate".to_owned()].into_iter().collect(),
        excluded_paths: ["my_app::io".to_owned()].into_iter().collect(),
        unknown_keys: Vec::new(),
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

#[test]
fn load_configuration_returns_config_when_present() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config()
        .returning(|_| Ok(Some(excluded_configuration())));

    let load = load_configuration_with_reader(&mock);
    assert_eq!(
        load.config.excluded_crates,
        HashSet::from(["my_crate".to_owned()]),
        "a readable configuration should retain its crate exclusions"
    );
    assert_eq!(
        load.config.excluded_paths,
        HashSet::from(["my_app::io".to_owned()]),
        "a readable configuration should retain its path exclusions"
    );
    assert!(
        load.issues.is_empty(),
        "a readable configuration should report no issues"
    );
}

/// Report the unknown keys without discarding the exclusions around them.
#[test]
fn load_configuration_keeps_known_exclusions_when_keys_are_unknown() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config().returning(|_| {
        Ok(Some(BoundaryConfig {
            excluded_crates: ["helper_crate".to_owned()].into_iter().collect(),
            excluded_paths: ["my_app::legacy_io".to_owned()].into_iter().collect(),
            unknown_keys: vec!["future_option".to_owned()],
        }))
    });

    let load = load_configuration_with_reader(&mock);
    assert_eq!(
        load.config.excluded_crates,
        HashSet::from(["helper_crate".to_owned()]),
        "an unknown key should not discard the crate exclusions"
    );
    assert_eq!(
        load.config.excluded_paths,
        HashSet::from(["my_app::legacy_io".to_owned()]),
        "an unknown key should not discard the path exclusions"
    );
    assert_eq!(
        load.issues,
        [ConfigIssue::UnknownKeys(vec!["future_option".to_owned()])],
        "the ignored key should be reported as an issue"
    );
}

#[test]
fn load_configuration_returns_default_when_none() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config().returning(|_| Ok(None));

    let load = load_configuration_with_reader(&mock);
    assert!(
        load.config.excluded_crates.is_empty(),
        "an absent configuration should exclude no crates"
    );
    assert!(
        load.config.excluded_paths.is_empty(),
        "an absent configuration should exclude no paths"
    );
    assert!(
        load.issues.is_empty(),
        "an absent configuration should report no issues"
    );
}

#[test]
fn load_configuration_returns_default_on_error() {
    let mut mock = MockConfigReader::new();
    mock.expect_read_config().returning(|_| {
        Err(Box::new(io::Error::other("parse error"))
            as Box<dyn std::error::Error + Send + Sync + 'static>)
    });

    let load = load_configuration_with_reader(&mock);
    assert!(
        load.config.excluded_crates.is_empty(),
        "a discarded section should leave no exclusions in force"
    );
    assert_eq!(
        load.issues,
        [ConfigIssue::SectionDiscarded("parse error".to_owned())],
        "a read error should be reported as a discarded section"
    );
}
