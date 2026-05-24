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
    let mut normalized = Vec::new();
    for attribute in attributes
        .into_iter()
        .map(|attribute| attribute.trim().trim_start_matches("rstest::").to_owned())
        .filter(|attribute| !attribute.is_empty())
    {
        if !normalized.contains(&attribute) {
            normalized.push(attribute);
        }
    }

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
#[path = "driver_tests.rs"]
mod tests;
