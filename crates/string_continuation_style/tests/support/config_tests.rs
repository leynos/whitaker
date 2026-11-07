//! Tests for the configuration schema and the loading fallbacks.

use std::collections::HashSet;

use super::{
    ApplicabilityMode,
    ConfigReader,
    StringContinuationStyleConfig,
    load_configuration_with_reader,
};

struct StubReader {
    result: Result<Option<StringContinuationStyleConfig>, String>,
}

impl ConfigReader for StubReader {
    fn read_config(
        &self,
        _lint_name: &str,
    ) -> Result<Option<StringContinuationStyleConfig>, Box<dyn std::error::Error + Send + Sync>>
    {
        match &self.result {
            Ok(config) => Ok(config.clone()),
            Err(message) => Err(message.clone().into()),
        }
    }
}

fn config() -> StringContinuationStyleConfig {
    StringContinuationStyleConfig {
        applicability: ApplicabilityMode::Machine,
        excluded_crates: HashSet::from([String::from("my_cli_app")]),
        excluded_paths: HashSet::from([String::from("my_app::legacy_text")]),
    }
}

#[test]
fn absence_of_a_section_yields_defaults() {
    let reader = StubReader { result: Ok(None) };
    let loaded = load_configuration_with_reader(&reader);

    assert_eq!(loaded.applicability, ApplicabilityMode::Manual);
    assert!(loaded.excluded_crates.is_empty());
}

#[test]
fn a_parse_failure_yields_defaults() {
    let reader = StubReader {
        result: Err(String::from("bad toml")),
    };

    assert_eq!(
        load_configuration_with_reader(&reader).applicability,
        ApplicabilityMode::Manual
    );
}

#[test]
fn a_present_section_is_returned_unchanged() {
    let reader = StubReader {
        result: Ok(Some(config())),
    };
    let loaded = load_configuration_with_reader(&reader);

    assert_eq!(loaded.applicability, ApplicabilityMode::Machine);
    assert!(loaded.is_excluded_crate("my_cli_app"));
    assert!(!loaded.is_excluded_crate("other_crate"));
}

#[test]
fn excluded_paths_build_a_working_exclusion_set() {
    let excluded = config().path_exclusions();

    assert!(excluded.excludes(&whitaker_common::SimplePath::parse(
        "my_app::legacy_text::inner"
    )));
    assert!(!excluded.excludes(&whitaker_common::SimplePath::parse("my_app::network")));
}

// The tests above inject an already-built configuration, so they cannot prove
// the `Deserialize` attributes work. These parse real TOML, because the schema
// is only ever reached through `dylint.toml`: `#[serde(deny_unknown_fields)]`
// and `deserialize_string_set` are behaviour no constructed value exercises.

fn parse(source: &str) -> Result<StringContinuationStyleConfig, toml::de::Error> {
    toml::from_str(source)
}

#[test]
fn both_applicability_spellings_parse() {
    let manual = parse(r#"applicability = "manual""#).expect("`manual` is a valid spelling");
    let machine = parse(r#"applicability = "machine""#).expect("`machine` is a valid spelling");

    assert_eq!(manual.applicability, ApplicabilityMode::Manual);
    assert_eq!(machine.applicability, ApplicabilityMode::Machine);
}

#[test]
fn an_omitted_applicability_defaults_to_manual() {
    let parsed = parse("").expect("an empty section is valid");

    assert_eq!(parsed.applicability, ApplicabilityMode::Manual);
    assert!(parsed.excluded_crates.is_empty());
    assert!(parsed.excluded_paths.is_empty());
}

#[test]
fn omission_of_each_field_is_independently_defaulted() {
    let only_crates =
        parse(r#"excluded_crates = ["my_cli_app"]"#).expect("a lone crate list is valid");

    assert_eq!(only_crates.applicability, ApplicabilityMode::Manual);
    assert_eq!(
        only_crates.excluded_crates,
        HashSet::from([String::from("my_cli_app")])
    );
    assert!(only_crates.excluded_paths.is_empty());
}

#[test]
fn unknown_fields_are_rejected() {
    let error = parse(
        r#"applicability = "machine"
unrecognised_key = true"#,
    )
    .expect_err("`deny_unknown_fields` must reject a mistyped key");

    assert!(
        error.to_string().contains("unrecognised_key"),
        "the error should name the offending key, got: {error}"
    );
}

#[test]
fn an_unknown_applicability_value_is_rejected() {
    let error = parse(r#"applicability = "automatic""#)
        .expect_err("only `manual` and `machine` are accepted");

    assert!(
        error.to_string().contains("automatic"),
        "the error should name the offending value, got: {error}"
    );
}

#[test]
fn a_non_array_exclusion_value_is_rejected() {
    parse(r#"excluded_crates = "my_cli_app""#)
        .expect_err("a bare string is not an array of crate names");
}

#[test]
fn non_string_exclusion_entries_are_rejected() {
    parse(r#"excluded_crates = ["my_cli_app", 7]"#)
        .expect_err("every exclusion entry must be a string");
    parse(r#"excluded_paths = [["nested"]]"#).expect_err("a nested array is not a module path");
}

#[test]
fn malformed_exclusion_arrays_are_rejected() {
    parse("excluded_paths = [").expect_err("an unterminated array is not valid TOML");
    parse(r#"excluded_paths = ["unterminated"#).expect_err("an unterminated string is invalid");
}

#[test]
fn exclusion_sets_parse_into_the_fields_they_name() {
    let parsed = parse(
        r#"
excluded_crates = ["my_cli_app", "my_lib"]
excluded_paths = ["my_app::legacy_text"]
"#,
    )
    .expect("both exclusion lists are valid");

    assert_eq!(
        parsed.excluded_crates,
        HashSet::from([String::from("my_cli_app"), String::from("my_lib")])
    );
    assert_eq!(
        parsed.excluded_paths,
        HashSet::from([String::from("my_app::legacy_text")])
    );
}
