//! Configuration parsing and loading for the bumpy road lint.
//!
//! The lint reads optional configuration from `dylint.toml`, applies defaults,
//! and relies on `analysis::normalise_settings` to clamp invalid values.

use crate::analysis::{Settings, Weights};
use log::debug;
use serde::Deserialize;

use super::LINT_NAME;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Config {
    threshold: f64,
    window: usize,
    min_bump_lines: usize,
    include_closures: bool,
    weights: WeightsConfig,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WeightsConfig {
    depth: f64,
    predicate: f64,
    flow: f64,
}

impl Default for WeightsConfig {
    fn default() -> Self {
        let defaults = Settings::default().weights;
        Self {
            depth: defaults.depth,
            predicate: defaults.predicate,
            flow: defaults.flow,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let defaults = Settings::default();
        Self {
            threshold: defaults.threshold,
            window: defaults.window,
            min_bump_lines: defaults.min_bump_lines,
            include_closures: defaults.include_closures,
            weights: WeightsConfig::default(),
        }
    }
}

impl Config {
    pub(super) fn into_settings(self) -> Settings {
        Settings {
            threshold: self.threshold,
            window: self.window,
            min_bump_lines: self.min_bump_lines,
            include_closures: self.include_closures,
            weights: Weights {
                depth: self.weights.depth,
                predicate: self.weights.predicate,
                flow: self.weights.flow,
            },
        }
    }
}

pub(super) fn load_configuration() -> Config {
    match dylint_linting::config::<Config>(LINT_NAME) {
        Ok(Some(config)) => config,
        Ok(None) => Config::default(),
        Err(error) => {
            debug!(
                target: LINT_NAME,
                "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
            );
            Config::default()
        }
    }
}
