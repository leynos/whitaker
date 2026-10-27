//! Persisted application settings and their defaults.

use crate::lint_config::{
    BumpyRoad, ConditionalMaxBranches, ModuleMaxLines, NoStdFs, NoUnwrapOrElsePanic, RstestHelper,
    TestAttributes,
};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Hold settings loaded by the application before domain validation.
#[derive(Clone, Debug, Deserialize, Serialize, OrthoConfig)]
#[serde(default, deny_unknown_fields)]
#[ortho_config(prefix = "WHITAKER_")]
pub struct Configuration {
    /// Select the lint bundles to run.
    #[ortho_config(skip_cli)]
    pub lint: Lint,
    /// Control installation policy.
    #[ortho_config(skip_cli)]
    pub install: Install,
    /// Select diagnostic presentation.
    #[ortho_config(skip_cli)]
    pub diagnostics: Diagnostics,
    /// Select output presentation.
    #[ortho_config(skip_cli)]
    pub output: Output,
    /// Module line-count policy.
    #[ortho_config(skip_cli)]
    pub module_max_lines: ModuleMaxLines,
    /// Conditional branch-count policy.
    #[ortho_config(skip_cli)]
    pub conditional_max_n_branches: ConditionalMaxBranches,
    /// Additional test attributes for the expect lint.
    #[ortho_config(skip_cli)]
    pub no_expect_outside_tests: TestAttributes,
    /// Additional test attributes for the example lint.
    #[ortho_config(skip_cli)]
    pub test_must_not_have_example: TestAttributes,
    /// Panic-in-main policy.
    #[ortho_config(skip_cli)]
    pub no_unwrap_or_else_panic: NoUnwrapOrElsePanic,
    /// Fixture extraction policy.
    #[ortho_config(skip_cli)]
    pub rstest_helper_should_be_fixture: RstestHelper,
    /// Filesystem exclusion policy.
    #[ortho_config(skip_cli)]
    pub no_std_fs_operations: NoStdFs,
    /// Complexity scoring policy.
    #[ortho_config(skip_cli)]
    pub bumpy_road_function: BumpyRoad,
    /// Deferred command-specific configuration namespaces.
    #[ortho_config(skip_cli)]
    pub cmds: BTreeMap<String, Value>,
}

impl Default for Configuration {
    /// Supply the documented application defaults.
    fn default() -> Self {
        Self {
            lint: Lint::default(),
            install: Install::default(),
            diagnostics: Diagnostics::default(),
            output: Output::default(),
            module_max_lines: ModuleMaxLines::default(),
            conditional_max_n_branches: ConditionalMaxBranches::default(),
            no_expect_outside_tests: TestAttributes::default(),
            test_must_not_have_example: TestAttributes::default(),
            no_unwrap_or_else_panic: NoUnwrapOrElsePanic::default(),
            rstest_helper_should_be_fixture: RstestHelper::default(),
            no_std_fs_operations: NoStdFs::default(),
            bumpy_road_function: BumpyRoad::default(),
            cmds: BTreeMap::new(),
        }
    }
}

/// Configure lint selection.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Lint {
    /// Include these selectors.
    pub select: Vec<String>,
    /// Exclude these selectors.
    pub ignore: Vec<String>,
    /// Enable experimental lints.
    pub experimental: bool,
}

impl Default for Lint {
    /// Supply the documented lint defaults.
    fn default() -> Self {
        Self {
            select: vec![String::from("DEFAULT")],
            ignore: Vec::new(),
            experimental: false,
        }
    }
}

/// Configure installation policy.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Install {
    /// Install only when necessary.
    pub lazy: bool,
    /// Prefer a prebuilt distribution.
    pub prefer_prebuilt: bool,
    /// Permit building from source.
    pub allow_build_from_source: bool,
}

impl Default for Install {
    /// Supply the documented installation defaults.
    fn default() -> Self {
        Self {
            lazy: true,
            prefer_prebuilt: true,
            allow_build_from_source: true,
        }
    }
}

/// Configure diagnostic language.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Diagnostics {
    /// Select the requested locale.
    pub locale: String,
}

impl Default for Diagnostics {
    /// Supply the fallback locale.
    fn default() -> Self {
        Self {
            locale: String::from("en-GB"),
        }
    }
}

/// Configure display output.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Output {
    /// Select colour use.
    pub colour: String,
    /// Select progress display.
    pub progress: String,
}

impl Default for Output {
    /// Supply the documented output defaults.
    fn default() -> Self {
        Self {
            colour: String::from("auto"),
            progress: String::from("plain"),
        }
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
