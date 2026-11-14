//! Configuration for the `no_std_fs_operations` lint: the settings schema and
//! the `dylint.toml` loading path, kept separate from the lint pass itself.

use std::{collections::HashSet, fmt};

use serde::{
    Deserialize,
    de::{IgnoredAny, MapAccess, Visitor},
};

use crate::exclusion::PathExclusions;

/// Lint name used for `dylint.toml` lookups, diagnostic codes, and log targets.
pub(crate) const LINT_NAME: &str = "no_std_fs_operations";

/// Package version embedded in configuration warnings.
pub(crate) const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Revision of the compiled Whitaker suite, when the build supplied one.
const SUITE_REVISION: Option<&str> = option_env!("WHITAKER_SUITE_REVISION");

/// Return the suite revision, or `"unknown"` when the build did not embed one.
///
/// Source builds and release packaging set `WHITAKER_SUITE_REVISION` from the
/// checked-out commit. A build that cannot resolve a revision keeps linting,
/// but its diagnostics must say so rather than inventing provenance.
#[must_use]
pub(crate) fn suite_revision() -> &'static str { SUITE_REVISION.unwrap_or("unknown") }

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
/// # Validation Contracts
///
/// Deserializing `NoStdFsConfig` directly—as callers embedding the type in
/// their own configuration do—uses `deny_unknown_fields`, so an unrecognized
/// key is an error.
///
/// The Dylint loader deliberately does *not* deserialize directly into this
/// type. It uses a private boundary that understands the keys this suite
/// supports, records any unknown top-level keys, and still applies the known
/// exclusions. That keeps a newer `dylint.toml` forward-compatible with an
/// older rolling suite instead of silently discarding every exclusion. The
/// driver reports the ignored keys through a Cargo-visible warning.
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

/// The lint's `dylint.toml` table as seen by the Dylint boundary.
///
/// This type is intentionally stricter than "parse anything": the keys the
/// suite knows are decoded with their real types, duplicate known fields are
/// rejected, and any other top-level key is recorded by name while its value
/// is consumed. The value is ignored rather than decoded because an older
/// suite cannot know how to interpret a newer key's shape.
#[derive(Debug, Default)]
pub(crate) struct BoundaryConfig {
    excluded_crates: HashSet<String>,
    excluded_paths: HashSet<String>,
    unknown_keys: Vec<String>,
}

impl BoundaryConfig {
    /// Split the boundary view into the effective configuration and ignored
    /// top-level key names, sorted and deduplicated.
    pub(crate) fn into_parts(self) -> (NoStdFsConfig, Vec<String>) {
        let config = NoStdFsConfig {
            excluded_crates: self.excluded_crates,
            excluded_paths: self.excluded_paths,
        };
        (config, self.unknown_keys)
    }
}

impl<'de> Deserialize<'de> for BoundaryConfig {
    /// Deserialize the table through the boundary visitor.
    ///
    /// # Errors
    ///
    /// Returns the deserializer's error when the input is not a map or a
    /// known value does not match its declared type.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(BoundaryConfigVisitor)
    }
}

/// Visitor that decodes known `no_std_fs_operations` keys and records unknown
/// top-level names.
struct BoundaryConfigVisitor;

impl<'de> Visitor<'de> for BoundaryConfigVisitor {
    type Value = BoundaryConfig;

    /// Describe the shape this visitor accepts in serde error messages.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `[no_std_fs_operations]` configuration table")
    }

    /// Decode the known keys and record the rest by name.
    ///
    /// Unknown values are consumed with `IgnoredAny` because an older suite
    /// cannot know how to interpret a newer key's shape.
    ///
    /// # Errors
    ///
    /// Returns the map access error for a malformed known value, or
    /// `duplicate_field` when a known key appears twice.
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut excluded_crates: Option<HashSet<String>> = None;
        let mut excluded_paths: Option<HashSet<String>> = None;
        let mut unknown_keys: Vec<String> = Vec::new();

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "excluded_crates" => {
                    decode_known_field(&mut map, &mut excluded_crates, "excluded_crates")?;
                }
                "excluded_paths" => {
                    decode_known_field(&mut map, &mut excluded_paths, "excluded_paths")?;
                }
                unknown => {
                    // Consume the value without decoding it: a newer suite may
                    // use a scalar, array, table, date, or time here.
                    map.next_value::<IgnoredAny>()?;
                    unknown_keys.push(unknown.to_owned());
                }
            }
        }

        unknown_keys.sort_unstable();
        unknown_keys.dedup();

        Ok(BoundaryConfig {
            excluded_crates: excluded_crates.unwrap_or_default(),
            excluded_paths: excluded_paths.unwrap_or_default(),
            unknown_keys,
        })
    }
}

/// Decode one known string-array field into its slot, rejecting a repeat.
///
/// TOML itself rejects a duplicate key, but a deserializer may still present
/// one; rejecting it here keeps the boundary as strict as the public
/// `deny_unknown_fields` type for the keys it understands.
fn decode_known_field<'de, A>(
    map: &mut A,
    slot: &mut Option<HashSet<String>>,
    field: &'static str,
) -> Result<(), A::Error>
where
    A: MapAccess<'de>,
{
    if slot.is_some() {
        return Err(serde::de::Error::duplicate_field(field));
    }
    *slot = Some(map.next_value::<Vec<String>>()?.into_iter().collect());
    Ok(())
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

/// A problem discovered while loading the lint's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConfigIssue {
    /// Top-level keys this suite does not understand; the known keys still
    /// apply.
    UnknownKeys(Vec<String>),
    /// The whole table was discarded because a known value did not parse.
    SectionDiscarded(String),
}

/// The effective configuration plus any problems worth reporting.
#[derive(Clone, Debug, Default)]
pub(crate) struct ConfigLoad {
    /// Configuration the lint should apply.
    pub(crate) config: NoStdFsConfig,
    /// Problems to report through a Cargo-visible warning.
    pub(crate) issues: Vec<ConfigIssue>,
}

/// Trait for loading lint configuration, enabling dependency injection for tests.
///
/// # Return Values
///
/// - `Ok(Some(boundary))` - Configuration section found; known keys decoded and unknown top-level
///   keys recorded
/// - `Ok(None)` - No configuration file or section present
/// - `Err(...)` - Configuration file exists but a known value failed to parse
///
/// # Example Usage (in tests)
///
/// ```text
/// let mut mock = MockConfigReader::new();
/// mock.expect_read_config()
///     .returning(|_| Ok(Some(BoundaryConfig::default())));
///
/// let load = load_configuration_with_reader(&mock);
/// assert!(load.config.excluded_crates.is_empty());
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
    ) -> Result<Option<BoundaryConfig>, Box<dyn std::error::Error + Send + Sync + 'static>>;
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
    ) -> Result<Option<BoundaryConfig>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
        dylint_linting::config::<BoundaryConfig>(lint_name)
            .map_err(|error| -> BoxError { Box::new(error) })
    }
}

/// Load lint configuration using the provided reader.
///
/// Returns the default configuration with no issues when no file or section
/// exists. A parsed section keeps every known exclusion and reports any
/// unknown keys as an issue. A parse error discards the section, reports the
/// error, and falls back to defaults.
///
/// # Example Return Cases
///
/// ```text
/// // Case 1: Configuration present
/// reader.read_config("lint") => Ok(Some(config))
/// load_configuration_with_reader(&reader) => config + unknown-key issues
///
/// // Case 2: No configuration section
/// reader.read_config("lint") => Ok(None)
/// load_configuration_with_reader(&reader) => NoStdFsConfig::default()
///
/// // Case 3: Parse error
/// reader.read_config("lint") => Err(...)
/// load_configuration_with_reader(&reader) => NoStdFsConfig::default() + issue
/// ```
pub(crate) fn load_configuration_with_reader(reader: &dyn ConfigReader) -> ConfigLoad {
    match reader.read_config(LINT_NAME) {
        Ok(Some(boundary)) => {
            let (config, unknown_keys) = boundary.into_parts();
            let issues = if unknown_keys.is_empty() {
                Vec::new()
            } else {
                vec![ConfigIssue::UnknownKeys(unknown_keys)]
            };
            ConfigLoad { config, issues }
        }
        Ok(None) => ConfigLoad::default(),
        Err(error) => ConfigLoad {
            config: NoStdFsConfig::default(),
            issues: vec![ConfigIssue::SectionDiscarded(error.to_string())],
        },
    }
}

/// Load lint configuration from `dylint.toml`.
///
/// Returns the default configuration with no issues when no file or section
/// exists, or the effective configuration plus any issues found while parsing.
pub(crate) fn load_configuration() -> ConfigLoad {
    load_configuration_with_reader(&DylintConfigReader)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
