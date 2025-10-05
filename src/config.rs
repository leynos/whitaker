//! Workspace-wide configuration loader backed by `dylint_linting`.
//!
//! The Whitaker suite keeps lint settings in `dylint.toml`, grouped by
//! package name. `SharedConfig` captures the subset of settings that apply to
//! the suite crate itself so that lints can reuse a single source of truth.
//! The loader defers to `dylint_linting::config_or_default` so that the
//! semantics match what Dylint expects: values are deserialised from
//! `dylint.toml` when present and fall back to sensible defaults otherwise.

use serde::Deserialize;

/// Shared configuration for the workspace-level crate.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct SharedConfig {
    /// Overrides for the `module_max_400_lines` lint. Keeping the field optional
    /// allows the lint to be configured without duplicating the default value in
    /// every `dylint.toml`.
    pub module_max_400_lines: ModuleMax400LinesConfig,
}

impl SharedConfig {
    /// Loads the configuration for the Whitaker suite.
    ///
    /// The key matches the crate name so that per-crate overrides in
    /// `dylint.toml` Just Work™ for both the aggregated suite and individual
    /// lint crates. `dylint_linting::config_or_default` panics when the stored
    /// data cannot be deserialised, mirroring Dylint's behaviour and surfacing
    /// misconfigured projects eagerly.
    #[must_use]
    pub fn load() -> Self {
        dylint_linting::config_or_default(env!("CARGO_PKG_NAME"))
    }
}

/// Settings that influence the forthcoming `module_max_400_lines` lint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct ModuleMax400LinesConfig {
    /// Maximum number of lines permitted per module before the lint fires.
    #[serde(default = "ModuleMax400LinesConfig::default_max_lines")]
    pub max_lines: usize,
}

impl ModuleMax400LinesConfig {
    const fn default_max_lines() -> usize {
        400
    }
}

impl Default for ModuleMax400LinesConfig {
    fn default() -> Self {
        Self {
            max_lines: Self::default_max_lines(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn defaults_match_the_suite_baseline() {
        let config = SharedConfig::default();

        assert_eq!(config.module_max_400_lines.max_lines, 400);
    }

    #[rstest]
    fn deserialises_overrides_from_toml() {
        let source = "[module_max_400_lines]\nmax_lines = 120\n";

        let config = match toml::from_str::<SharedConfig>(source) {
            Ok(value) => value,
            Err(error) => panic!("expected configuration to parse successfully: {error}"),
        };

        assert_eq!(config.module_max_400_lines.max_lines, 120);
    }

    #[rstest]
    fn propagates_deserialisation_failures() {
        let source = "[module_max_400_lines]\nmax_lines = \"a lot\"\n";

        let outcome: Result<SharedConfig, _> = toml::from_str(source);

        assert!(
            outcome.is_err(),
            "expected a parse error when max_lines is not numeric"
        );
    }
}
