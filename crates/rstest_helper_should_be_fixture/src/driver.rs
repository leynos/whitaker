//! Dylint driver bootstrap for the `rstest` helper fixture lint.
//!
//! The driver owns compiler integration and configuration loading. Pure
//! configuration normalization stays in small helper methods so it can be
//! tested without constructing rustc contexts.

use log::debug;
use rustc_lint::{LateContext, LateLintPass};
use serde::Deserialize;
use whitaker::SharedConfig;
use whitaker_common::attributes::AttributePath;
use whitaker_common::i18n::{Localizer, get_localizer_for_lint};
use whitaker_common::rstest::RstestDetectionOptions;

const LINT_NAME: &str = "rstest_helper_should_be_fixture";

const DEFAULT_PROVIDER_PARAM_ATTRIBUTES: &[&str] =
    &["case", "values", "files", "future", "context"];

type ConfigLoadResult = Result<Config, String>;

dylint_linting::impl_late_lint! {
    pub RSTEST_HELPER_SHOULD_BE_FIXTURE,
    Warn,
    "repeated rstest helper calls should be extracted into fixtures",
    RstestHelperShouldBeFixture::default()
}

/// Configuration for the `rstest_helper_should_be_fixture` lint.
///
/// Values are loaded from `dylint.toml` and normalized so threshold settings
/// keep repeated-helper semantics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Config {
    min_calls: usize,
    min_distinct_tests: usize,
    require_identical_fixture_arg_names: bool,
    provider_param_attributes: Vec<String>,
    use_source_callee_fallback: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_calls: 2,
            min_distinct_tests: 2,
            require_identical_fixture_arg_names: false,
            provider_param_attributes: DEFAULT_PROVIDER_PARAM_ATTRIBUTES
                .iter()
                .map(ToString::to_string)
                .collect(),
            use_source_callee_fallback: false,
        }
    }
}

impl Config {
    fn normalized(self) -> Self {
        Self {
            min_calls: self.min_calls.max(2),
            min_distinct_tests: self.min_distinct_tests.max(2),
            require_identical_fixture_arg_names: self.require_identical_fixture_arg_names,
            provider_param_attributes: normalize_provider_attributes(
                self.provider_param_attributes,
            ),
            use_source_callee_fallback: self.use_source_callee_fallback,
        }
    }

    fn detection_options(&self) -> RstestDetectionOptions {
        let provider_param_attributes = self
            .provider_param_attributes
            .iter()
            .flat_map(|attribute| {
                [
                    AttributePath::from(attribute.as_str()),
                    AttributePath::from(format!("rstest::{attribute}")),
                ]
            })
            .collect();
        RstestDetectionOptions::new(provider_param_attributes, self.use_source_callee_fallback)
    }
}

/// Lint pass bootstrap for repeated `rstest` helper extraction.
pub struct RstestHelperShouldBeFixture {
    config: Config,
    detection_options: RstestDetectionOptions,
    localizer: Localizer,
}

impl Default for RstestHelperShouldBeFixture {
    fn default() -> Self {
        let config = Config::default();
        let detection_options = config.detection_options();
        Self {
            config,
            detection_options,
            localizer: Localizer::new(None),
        }
    }
}

impl RstestHelperShouldBeFixture {
    fn apply_loaded_crate_configuration(
        &mut self,
        config: ConfigLoadResult,
        shared_config: SharedConfig,
    ) {
        let config = match config {
            Ok(config) => config,
            Err(error) => {
                debug!(
                    target: LINT_NAME,
                    "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
                );
                Config::default()
            }
        };

        self.apply_crate_configuration(config, shared_config);
    }

    fn apply_crate_configuration(&mut self, config: Config, shared_config: SharedConfig) {
        debug!(
            target: LINT_NAME,
            "applying `{LINT_NAME}` configuration: min_calls={}, min_distinct_tests={}, \
             require_identical_fixture_arg_names={}, provider_param_attributes={:?}, \
             use_source_callee_fallback={}, locale={:?}",
            config.min_calls,
            config.min_distinct_tests,
            config.require_identical_fixture_arg_names,
            config.provider_param_attributes,
            config.use_source_callee_fallback,
            shared_config.locale(),
        );
        self.config = config;
        self.detection_options = self.config.detection_options();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }
}

impl<'tcx> LateLintPass<'tcx> for RstestHelperShouldBeFixture {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        self.apply_loaded_crate_configuration(load_configuration(), load_shared_config());
    }
}

fn normalize_provider_attributes(attributes: Vec<String>) -> Vec<String> {
    let normalized: Vec<String> = attributes
        .into_iter()
        .map(|attribute| attribute.trim().trim_start_matches("rstest::").to_owned())
        .filter(|attribute| !attribute.is_empty())
        .collect();

    if normalized.is_empty() {
        return default_provider_param_attributes();
    }

    normalized
}

fn default_provider_param_attributes() -> Vec<String> {
    DEFAULT_PROVIDER_PARAM_ATTRIBUTES
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn load_configuration() -> ConfigLoadResult {
    debug!(target: LINT_NAME, "loading `{LINT_NAME}` configuration");
    loaded_configuration(dylint_linting::config::<Config>(LINT_NAME))
}

fn load_shared_config() -> SharedConfig {
    // SAFETY / NOTE: `SharedConfig::load` does not currently propagate I/O
    // errors, so this named boundary documents the infallible call site
    // pending https://github.com/leynos/whitaker/issues/233.
    debug!(target: LINT_NAME, "loading shared Whitaker configuration");
    SharedConfig::load()
}

fn loaded_configuration<E>(loaded: Result<Option<Config>, E>) -> ConfigLoadResult
where
    E: std::fmt::Display,
{
    match loaded {
        Ok(Some(config)) => {
            debug!(target: LINT_NAME, "loaded explicit `{LINT_NAME}` configuration");
            Ok(config.normalized())
        }
        Ok(None) => {
            debug!(target: LINT_NAME, "no `{LINT_NAME}` configuration found; using defaults");
            Ok(Config::default())
        }
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for driver configuration normalization, loading boundaries,
    //! and `rstest` detection option construction.
    //!
    //! NOTE: `SharedConfig::load` is treated as infallible at the driver call
    //! site pending https://github.com/leynos/whitaker/issues/233.

    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

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
}
