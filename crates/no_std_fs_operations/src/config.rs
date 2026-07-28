//! Configuration for the `no_std_fs_operations` lint: the settings schema and
//! the `dylint.toml` loading path, kept separate from the lint pass itself.

use crate::exclusion::PathExclusions;
use log::warn;
use serde::Deserialize;
use std::collections::HashSet;

/// Lint name used for `dylint.toml` lookups, diagnostic codes, and log targets.
pub(crate) const LINT_NAME: &str = "no_std_fs_operations";

/// Configuration for the `no_std_fs_operations` lint.
///
/// # TOML Configuration
///
/// In `dylint.toml` at your workspace root:
///
/// ```toml
/// [no_std_fs_operations]
/// excluded_crates = ["my_cli_app", "test_utilities"]
/// excluded_paths = ["my_app::legacy_io", "my_app::bin::migrate"]
/// ```
///
/// `excluded_crates` exempts an entire crate; `excluded_paths` exempts an
/// individual module (and everything nested beneath it). Both use Rust
/// identifiers (underscores), not Cargo package names (hyphens). Each
/// `excluded_paths` entry is a fully qualified path anchored at the crate
/// identifier and matches on segment boundaries, so `my_app::legacy_io`
/// exempts `my_app::legacy_io` and its descendants but never a sibling such as
/// `my_app::legacy_io_utils`.
///
/// # Strict Validation
///
/// This configuration uses `deny_unknown_fields`, meaning any unrecognized
/// key (such as a typo like `excluded_crate` instead of `excluded_crates`)
/// will cause configuration parsing to fail. When parsing fails, the lint
/// falls back to defaults and logs a warning. If exclusions don't work as
/// expected, check the logs for parse errors.
///
/// # Examples
///
/// ```rust
/// # use no_std_fs_operations::NoStdFsConfig;
/// # use std::collections::HashSet;
/// let config = NoStdFsConfig {
///     excluded_crates: HashSet::from(["my_cli_app".to_owned()]),
///     excluded_paths: HashSet::new(),
/// };
/// assert!(config.is_excluded("my_cli_app"));
/// assert!(!config.is_excluded("other_crate"));
/// ```
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NoStdFsConfig {
    /// Crate names excluded from the lint. These crates are allowed to use
    /// `std::fs` operations without triggering diagnostics.
    #[serde(deserialize_with = "deserialize_string_set")]
    pub excluded_crates: HashSet<String>,
    /// Module paths excluded from the lint. Items within these modules (and
    /// their descendants) are allowed to use `std::fs` operations without
    /// triggering diagnostics. Entries are fully qualified paths anchored at
    /// the crate identifier, matched on segment boundaries.
    #[serde(deserialize_with = "deserialize_string_set")]
    pub excluded_paths: HashSet<String>,
}

/// Deserialize a TOML array of strings into a `HashSet`.
///
/// Shared by `excluded_crates` and `excluded_paths` so both fields accept the
/// same `["a", "b"]` array form and reject non-array or non-string input.
fn deserialize_string_set<'de, D>(deserializer: D) -> Result<HashSet<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let vec: Vec<String> = Vec::deserialize(deserializer)?;
    Ok(vec.into_iter().collect())
}

impl NoStdFsConfig {
    /// Check if the given crate name is excluded from the lint.
    ///
    /// Returns `true` if `crate_name` appears in the `excluded_crates` set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use no_std_fs_operations::NoStdFsConfig;
    /// # use std::collections::HashSet;
    /// let config = NoStdFsConfig {
    ///     excluded_crates: HashSet::from(["my_cli".to_owned(), "test_utils".to_owned()]),
    ///     excluded_paths: HashSet::new(),
    /// };
    ///
    /// assert!(config.is_excluded("my_cli"));
    /// assert!(!config.is_excluded("other_crate"));
    /// ```
    #[must_use]
    pub fn is_excluded(&self, crate_name: &str) -> bool {
        self.excluded_crates.contains(crate_name)
    }

    /// Build the module-path exclusion set from `excluded_paths`.
    #[must_use]
    pub(crate) fn path_exclusions(&self) -> PathExclusions {
        PathExclusions::new(&self.excluded_paths)
    }
}

/// Trait for loading lint configuration, enabling dependency injection for tests.
///
/// # Return Values
///
/// - `Ok(Some(config))` - Configuration section found and parsed successfully
/// - `Ok(None)` - No configuration file or section present
/// - `Err(...)` - Configuration file exists but parsing failed
///
/// # Example Usage (in tests)
///
/// ```text
/// let mut mock = MockConfigReader::new();
/// mock.expect_read_config()
///     .returning(|_| Ok(Some(NoStdFsConfig {
///         excluded_crates: HashSet::from(["my_crate".to_owned()]),
///         excluded_paths: HashSet::new(),
///     })));
///
/// let config = load_configuration_with_reader(&mock);
/// assert!(config.is_excluded("my_crate"));
/// ```
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ConfigReader {
    /// Read configuration for the given lint name.
    ///
    /// Returns `Ok(Some(config))` if found, `Ok(None)` if not present, or
    /// `Err` if parsing fails.
    fn read_config(
        &self,
        lint_name: &str,
    ) -> Result<Option<NoStdFsConfig>, Box<dyn std::error::Error + Send + Sync + 'static>>;
}

/// Production implementation that reads from `dylint.toml` via `dylint_linting::config`.
///
/// This reader delegates to `dylint_linting::config` to locate and parse
/// the `dylint.toml` configuration file in the workspace root.
pub(crate) struct DylintConfigReader;

impl ConfigReader for DylintConfigReader {
    fn read_config(
        &self,
        lint_name: &str,
    ) -> Result<Option<NoStdFsConfig>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
        dylint_linting::config::<NoStdFsConfig>(lint_name).map_err(|e| -> BoxError { Box::new(e) })
    }
}

/// Load lint configuration using the provided reader.
///
/// Returns the default configuration when:
/// - No configuration file exists
/// - No `[no_std_fs_operations]` section is present
/// - The configuration fails to parse (logged at `warn` level)
///
/// # Example Return Cases
///
/// ```text
/// // Case 1: Configuration present
/// reader.read_config("lint") => Ok(Some(config))
/// load_configuration_with_reader(&reader) => config
///
/// // Case 2: No configuration section
/// reader.read_config("lint") => Ok(None)
/// load_configuration_with_reader(&reader) => NoStdFsConfig::default()
///
/// // Case 3: Parse error
/// reader.read_config("lint") => Err(...)
/// load_configuration_with_reader(&reader) => NoStdFsConfig::default() + logs warning
/// ```
pub(crate) fn load_configuration_with_reader(reader: &dyn ConfigReader) -> NoStdFsConfig {
    match reader.read_config(LINT_NAME) {
        Ok(Some(config)) => config,
        Ok(None) => NoStdFsConfig::default(),
        Err(error) => {
            warn!(
                target: LINT_NAME,
                "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
            );
            NoStdFsConfig::default()
        }
    }
}

/// Load lint configuration from `dylint.toml`.
///
/// Returns the default configuration when:
/// - No configuration file exists
/// - No `[no_std_fs_operations]` section is present
/// - The configuration fails to parse (logged at `warn` level)
pub(crate) fn load_configuration() -> NoStdFsConfig {
    load_configuration_with_reader(&DylintConfigReader)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
