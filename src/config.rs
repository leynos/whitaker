//! Workspace-wide configuration loader backed by `dylint_linting`.
//!
//! The Whitaker suite keeps lint settings in `dylint.toml`, grouped by
//! package name. `SharedConfig` captures the subset of settings that apply to
//! the suite crate itself so that lints can reuse a single source of truth.
//! The loader defers to `dylint_linting::config_or_default` so that the
//! semantics match what Dylint expects: values are deserialised from
//! `dylint.toml` when present and fall back to sensible defaults otherwise.

use common::i18n::normalise_locale;
use serde::Deserialize;

/// Shared configuration for the workspace-level crate.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SharedConfig {
    /// Preferred locale for Whitaker lints when the environment is silent.
    ///
    /// This optional override allows CI and editor integrations to pin a
    /// deterministic locale without relying on process-level environment
    /// variables. The resolver trims whitespace and ignores blank values, so
    /// configuration such as `locale = ""` falls back cleanly to the bundled
    /// default.
    pub locale: Option<String>,
    /// Overrides for the `module_max_lines` lint. This field falls back to
    /// its default when omitted from `dylint.toml`, which avoids duplicating the
    /// baseline settings in every workspace.
    pub module_max_lines: ModuleMaxLinesConfig,
}

impl SharedConfig {
    /// Loads the configuration for the Whitaker suite crate.
    ///
    /// This convenience method keeps the existing call sites simple while the
    /// [`Self::load_with`] variant allows downstream lint crates to resolve their own
    /// configuration namespace explicitly.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "dylint-driver")]
    /// # {
    /// use whitaker::SharedConfig;
    ///
    /// let config = SharedConfig::load();
    /// assert_eq!(config.module_max_lines.max_lines, 400);
    /// # }
    /// ```
    #[must_use]
    pub fn load() -> Self {
        #[cfg(feature = "dylint-driver")]
        {
            dylint_linting::config_or_default(env!("CARGO_PKG_NAME"))
        }

        #[cfg(not(feature = "dylint-driver"))]
        {
            panic!(
                "`SharedConfig::load` uses the Dylint loader; use `SharedConfig::load_with` to inject a stub when testing"
            );
        }
    }

    /// Loads configuration using the supplied loader.
    ///
    /// Each lint crate stores its overrides under a table matching its crate
    /// name (for example `[module_max_lines]`). The `crate_name` parameter
    /// ensures the loader resolves the caller's namespace explicitly. This
    /// helper also exists to support dependency injection in tests so that the
    /// behaviour of `dylint_linting::config_or_default` can be simulated without
    /// touching the file system.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::SharedConfig;
    ///
    /// let config = SharedConfig::load_with("whitaker", |_| SharedConfig::default());
    /// assert_eq!(config.module_max_lines.max_lines, 400);
    /// ```
    #[must_use]
    pub fn load_with<F>(crate_name: &str, loader: F) -> Self
    where
        F: FnOnce(&str) -> Self,
    {
        loader(crate_name)
    }

    /// Returns the configured locale override, if present.
    ///
    /// Whitespace-only values are treated as absent to avoid surprising
    /// behaviour when `dylint.toml` is templated or patched.
    #[must_use]
    pub fn locale(&self) -> Option<&str> {
        normalise_locale(self.locale.as_deref())
    }
}

/// Settings that influence the forthcoming `module_max_lines` lint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ModuleMaxLinesConfig {
    /// Maximum number of lines permitted per module before the lint fires.
    #[serde(default = "ModuleMaxLinesConfig::default_max_lines")]
    pub max_lines: usize,
}

impl ModuleMaxLinesConfig {
    const fn default_max_lines() -> usize {
        400
    }
}

impl Default for ModuleMaxLinesConfig {
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

        assert_eq!(config.module_max_lines.max_lines, 400);
        assert!(config.locale().is_none());
    }

    #[rstest]
    fn deserialises_overrides_from_toml() {
        let source = "[module_max_lines]\nmax_lines = 120\n";

        // Panic with the TOML parser's message so broken overrides are easy to debug.
        let config = toml::from_str::<SharedConfig>(source)
            .expect("expected configuration to parse successfully");

        assert_eq!(config.module_max_lines.max_lines, 120);
    }

    #[rstest]
    fn deserialises_locale_override() {
        let source = "locale = \"cy\"\n";

        let config = toml::from_str::<SharedConfig>(source)
            .expect("expected configuration to parse successfully");

        assert_eq!(config.locale(), Some("cy"));
    }

    #[rstest]
    fn trims_whitespace_only_locale_entries() {
        let source = "locale = \"  \"\n";

        let config = toml::from_str::<SharedConfig>(source)
            .expect("expected configuration to parse successfully");

        assert!(config.locale().is_none());
    }

    #[rstest]
    fn propagates_deserialisation_failures() {
        let source = "[module_max_lines]\nmax_lines = \"a lot\"\n";

        let outcome: Result<SharedConfig, _> = toml::from_str(source);

        assert!(
            outcome.is_err(),
            "expected a parse error when max_lines is not numeric"
        );
    }

    #[rstest]
    fn rejects_unknown_fields() {
        let source = concat!(
            "unexpected = true\n",
            "[module_max_lines]\n",
            "max_lines = 120\n",
        );

        let outcome: Result<SharedConfig, _> = toml::from_str(source);

        assert!(
            outcome.is_err(),
            "expected a parse error when unknown fields are present"
        );
    }

    #[rstest]
    fn load_with_passes_through_the_requested_crate() {
        fn stub_loader(crate_name: &str) -> SharedConfig {
            assert_eq!(crate_name, "module_max_lines");
            SharedConfig {
                locale: None,
                module_max_lines: ModuleMaxLinesConfig { max_lines: 123 },
            }
        }

        let config = SharedConfig::load_with("module_max_lines", stub_loader);

        assert_eq!(config.module_max_lines.max_lines, 123);
    }
}
