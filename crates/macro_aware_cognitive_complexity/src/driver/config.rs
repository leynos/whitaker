//! Configuration schema and loading for the macro-aware complexity lint.
//!
//! The lint pass reaches `dylint.toml` only through [`ConfigReader`], so the
//! fallback taken for an unreadable document is exercised by unit tests without
//! a compiler or an ambient configuration file. Production wires
//! [`DylintConfigReader`]; tests supply a mocked reader.

use log::warn;
use serde::Deserialize;

use super::LINT_NAME;

/// Highest authored complexity a body may reach without a diagnostic.
pub(super) const DEFAULT_MAX_COMPLEXITY: u64 = 25;

/// Parsed `dylint.toml` configuration for this lint.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Config {
    /// Limit above which a body's authored complexity is reported.
    pub(super) max_complexity: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_complexity: DEFAULT_MAX_COMPLEXITY,
        }
    }
}

/// Read this lint's configuration from a `dylint.toml` source.
///
/// The seam keeps the fallback path testable: a lint pass that called
/// `dylint_linting::config` directly could only be exercised with a real
/// configuration file present.
#[cfg_attr(test, mockall::automock)]
pub(super) trait ConfigReader {
    /// Read configuration for the given lint name.
    ///
    /// Returns `Ok(None)` when no configuration section exists, which is not an
    /// error: the lint then uses its defaults.
    ///
    /// # Errors
    ///
    /// Returns an error when a configuration section exists but cannot be
    /// parsed, so the caller can make the fallback visible.
    fn read_config(
        &self,
        lint_name: &str,
    ) -> Result<Option<Config>, Box<dyn std::error::Error + Send + Sync + 'static>>;
}

/// Production reader backed by `dylint_linting::config`.
struct DylintConfigReader;

impl ConfigReader for DylintConfigReader {
    fn read_config(
        &self,
        lint_name: &str,
    ) -> Result<Option<Config>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
        dylint_linting::config::<Config>(lint_name).map_err(|error| -> BoxError { Box::new(error) })
    }
}

/// Load the lint configuration through `reader`, defaulting on absence or error.
///
/// A parse failure is logged at warning level rather than debug: a mistyped key
/// otherwise silently substitutes the default limit, and the operator who
/// mistyped it is the only one who can fix it.
pub(super) fn load_configuration_with_reader(reader: &dyn ConfigReader) -> Config {
    match reader.read_config(LINT_NAME) {
        Ok(Some(config)) => config,
        Ok(None) => Config::default(),
        Err(error) => {
            warn!(
                target: LINT_NAME,
                "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
            );
            Config::default()
        }
    }
}

/// Load the lint configuration from `dylint.toml`.
pub(super) fn load_configuration() -> Config { load_configuration_with_reader(&DylintConfigReader) }

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
