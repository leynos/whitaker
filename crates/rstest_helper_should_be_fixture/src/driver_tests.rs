//! Unit tests for driver configuration normalization, loading boundaries, and
//! `rstest` detection option construction.
//!
//! NOTE: `SharedConfig::load` is treated as infallible at the driver call site
//! pending https://github.com/leynos/whitaker/issues/233.

use proptest::prelude::*;
use rstest::rstest;
use whitaker::SharedConfig;

use super::*;

#[rstest]
fn default_configuration_matches_design() {
    let config = Config::default();

    assert_eq!(config.min_calls, 2);
    assert_eq!(config.min_distinct_tests, 2);
    assert!(!config.require_identical_fixture_arg_names);
    assert_eq!(
        config.provider_param_attributes,
        ["case", "values", "files", "future", "context"]
    );
    assert!(!config.use_source_callee_fallback);
}

#[rstest]
fn deserializes_valid_configuration() {
    let config: Config = toml::from_str::<Config>(
        r#"
        min_calls = 3
        min_distinct_tests = 4
        require_identical_fixture_arg_names = true
        provider_param_attributes = ["case", "custom_provider"]
        use_source_callee_fallback = true
        "#,
    )
    .expect("valid configuration should deserialize")
    .normalized();

    assert_eq!(config.min_calls, 3);
    assert_eq!(config.min_distinct_tests, 4);
    assert!(config.require_identical_fixture_arg_names);
    assert_eq!(
        config.provider_param_attributes,
        ["case", "custom_provider"]
    );
    assert!(config.use_source_callee_fallback);
}

#[rstest]
fn rejects_unknown_configuration_fields() {
    let result = toml::from_str::<Config>("unexpected = true");

    assert!(result.is_err());
}

#[rstest]
fn normalizes_numeric_thresholds_to_two() {
    let config = Config {
        min_calls: 0,
        min_distinct_tests: 0,
        ..Config::default()
    }
    .normalized();

    assert_eq!(config.min_calls, 2);
    assert_eq!(config.min_distinct_tests, 2);
}

#[rstest]
#[case::plain(vec!["case".to_string()], vec!["case"])]
#[case::qualified(vec!["rstest::values".to_string()], vec!["values"])]
#[case::blank(vec![" ".to_string()], vec!["case", "values", "files", "future", "context"])]
fn normalizes_provider_attributes(#[case] input: Vec<String>, #[case] expected: Vec<&str>) {
    let normalized = normalize_provider_attributes(input);
    let expected: Vec<String> = expected.into_iter().map(ToString::to_string).collect();

    assert_eq!(normalized, expected);
}

#[rstest]
fn detection_options_expand_plain_and_rstest_qualified_provider_paths() {
    let config = Config {
        provider_param_attributes: vec!["case".to_string(), "custom".to_string()],
        use_source_callee_fallback: true,
        ..Config::default()
    };
    let options = config.detection_options();
    let paths: Vec<String> = options
        .provider_param_attributes()
        .iter()
        .map(ToString::to_string)
        .collect();

    assert_eq!(paths, ["case", "rstest::case", "custom", "rstest::custom"]);
    assert!(options.use_expansion_trace_fallback());
}

#[rstest]
fn lint_pass_default_derives_detection_options_from_config() {
    let pass = RstestHelperShouldBeFixture::default();

    assert_eq!(pass.config, Config::default());
    assert_eq!(
        pass.detection_options.provider_param_attributes().len(),
        DEFAULT_PROVIDER_PARAM_ATTRIBUTES.len() * 2
    );
}

#[rstest]
fn loaded_configuration_uses_default_when_config_is_absent() {
    assert_eq!(
        loaded_configuration::<String>(Ok(None)).expect("missing config should default"),
        Config::default(),
    );
}

#[rstest]
fn loaded_configuration_returns_error_when_config_errors() {
    assert_eq!(
        loaded_configuration(Err("invalid config")).expect_err("invalid config should error"),
        "invalid config",
    );
}

#[rstest]
fn loaded_configuration_normalizes_present_config() {
    let config = Config {
        min_calls: 1,
        min_distinct_tests: 1,
        provider_param_attributes: vec!["rstest::case".to_string()],
        ..Config::default()
    };

    assert_eq!(
        loaded_configuration::<String>(Ok(Some(config)))
            .expect("present config should load")
            .provider_param_attributes,
        ["case"]
    );
}

#[rstest]
fn applying_crate_configuration_initializes_pass_state() {
    let mut pass = RstestHelperShouldBeFixture::default();
    let config = Config {
        provider_param_attributes: vec!["custom".to_string()],
        use_source_callee_fallback: true,
        ..Config::default()
    };

    pass.apply_crate_configuration(config.clone(), SharedConfig::default());

    assert_eq!(pass.config, config);
    assert!(pass.detection_options.use_expansion_trace_fallback());
    assert_eq!(pass.detection_options.provider_param_attributes().len(), 2);
}

#[rstest]
fn check_crate_configuration_loads_and_normalizes_config() {
    let mut pass = RstestHelperShouldBeFixture::default();
    let config = Config {
        min_calls: 0,
        min_distinct_tests: 1,
        provider_param_attributes: vec!["rstest::case".to_string()],
        use_source_callee_fallback: true,
        ..Config::default()
    };

    pass.apply_loaded_crate_configuration(
        loaded_configuration::<String>(Ok(Some(config))),
        SharedConfig::default(),
    );

    assert_eq!(pass.config.min_calls, 2);
    assert_eq!(pass.config.min_distinct_tests, 2);
    assert_eq!(pass.config.provider_param_attributes, ["case"]);
    assert!(pass.detection_options.use_expansion_trace_fallback());
    assert_eq!(pass.detection_options.provider_param_attributes().len(), 2);
}

proptest! {
    #[test]
    fn normalized_configuration_is_idempotent(
        min_calls in 0usize..8,
        min_distinct_tests in 0usize..8,
        require_identical_fixture_arg_names in any::<bool>(),
        provider_param_attributes in prop::collection::vec("[ a-z:]{0,24}", 0..8),
        use_source_callee_fallback in any::<bool>(),
    ) {
        let config = Config {
            min_calls,
            min_distinct_tests,
            require_identical_fixture_arg_names,
            provider_param_attributes,
            use_source_callee_fallback,
        };
        let once = config.normalized();
        let twice = once.clone().normalized();

        prop_assert_eq!(once, twice);
    }
}
