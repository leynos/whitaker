//! Configuration for the `string_continuation_style` lint.
//!
//! The settings schema and the `dylint.toml` loading path live here, separate
//! from the lint pass, so the pass only sees a resolved configuration value.

use std::collections::HashSet;

use log::warn;
use serde::Deserialize;

use crate::exclusion::PathExclusions;

/// Lint name used for `dylint.toml` lookups and log targets.
pub(crate) const LINT_NAME: &str = "string_continuation_style";

/// How the replacement suggestion is labelled.
///
/// `manual` keeps the rewrite on screen but disarms unattended writers, which
/// is the default until the `// run-rustfix` harness has proved itself over
/// real rewrites. `machine` opts into `cargo fix` applying it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApplicabilityMode {
    /// Emit the rewrite as `MaybeIncorrect`.
    #[default]
    Manual,
    /// Emit the rewrite as `MachineApplicable`.
    Machine,
}

/// Configuration for the `string_continuation_style` lint.
///
/// # TOML Configuration
///
/// In `dylint.toml` at your workspace root:
///
/// ```toml
/// [string_continuation_style]
/// applicability = "manual"
/// excluded_crates = ["my_cli_app"]
/// excluded_paths = ["my_app::legacy_text"]
/// ```
///
/// `excluded_crates` exempts an entire crate; `excluded_paths` exempts a module
/// and everything nested beneath it. Both use Rust identifiers (underscores),
/// not Cargo package names (hyphens).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StringContinuationStyleConfig {
    /// Whether the replacement is offered as a machine-applicable fix.
    pub applicability: ApplicabilityMode,
    /// Crate names whose literals are never diagnosed.
    #[serde(deserialize_with = "deserialize_string_set")]
    pub excluded_crates: HashSet<String>,
    /// Module paths whose literals are never diagnosed.
    #[serde(deserialize_with = "deserialize_string_set")]
    pub excluded_paths: HashSet<String>,
}

/// Deserializes a TOML array of strings into a `HashSet`.
///
/// Shared by `excluded_crates` and `excluded_paths` so both fields accept the
/// same `["a", "b"]` form and reject non-array or non-string input.
fn deserialize_string_set<'de, D>(deserializer: D) -> Result<HashSet<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values: Vec<String> = Vec::deserialize(deserializer)?;
    Ok(values.into_iter().collect())
}

impl StringContinuationStyleConfig {
    /// Returns `true` when `crate_name` is excluded from the lint.
    #[must_use]
    pub fn is_excluded_crate(&self, crate_name: &str) -> bool {
        self.excluded_crates.contains(crate_name)
    }

    /// Builds the module-path exclusion set from `excluded_paths`.
    #[must_use]
    pub(crate) fn path_exclusions(&self) -> PathExclusions {
        PathExclusions::new(&self.excluded_paths)
    }
}

/// Reads the lint's configuration section, enabling dependency injection.
///
/// `Ok(None)` means no section was configured, which is not an error.
pub(crate) trait ConfigReader {
    /// Reads configuration for the given lint name.
    fn read_config(
        &self,
        lint_name: &str,
    ) -> Result<Option<StringContinuationStyleConfig>, Box<dyn std::error::Error + Send + Sync>>;
}

/// Production reader backed by `dylint.toml` via `dylint_linting::config`.
pub(crate) struct DylintConfigReader;

impl ConfigReader for DylintConfigReader {
    fn read_config(
        &self,
        lint_name: &str,
    ) -> Result<Option<StringContinuationStyleConfig>, Box<dyn std::error::Error + Send + Sync>>
    {
        type BoxError = Box<dyn std::error::Error + Send + Sync>;
        dylint_linting::config::<StringContinuationStyleConfig>(lint_name)
            .map_err(|error| -> BoxError { Box::new(error) })
    }
}

/// Loads the configuration, falling back to defaults on absence or failure.
///
/// A parse failure is logged and replaced by defaults rather than aborting the
/// compilation: a mistyped key must not break a user's build.
pub(crate) fn load_configuration_with_reader(
    reader: &dyn ConfigReader,
) -> StringContinuationStyleConfig {
    match reader.read_config(LINT_NAME) {
        Ok(Some(config)) => config,
        Ok(None) => StringContinuationStyleConfig::default(),
        Err(error) => {
            warn!(
                target: LINT_NAME,
                "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
            );
            StringContinuationStyleConfig::default()
        }
    }
}

/// Loads the lint configuration from `dylint.toml`.
pub(crate) fn load_configuration() -> StringContinuationStyleConfig {
    load_configuration_with_reader(&DylintConfigReader)
}

#[cfg(test)]
#[path = "../tests/support/config_tests.rs"]
mod tests;
