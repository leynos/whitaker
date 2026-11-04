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
