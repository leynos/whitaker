//! Typed settings for existing Whitaker lints.

use serde::{Deserialize, Serialize};

/// Configure the maximum number of lines in a module.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModuleMaxLines {
    /// Maximum number of lines.
    pub max_lines: usize,
}
impl Default for ModuleMaxLines {
    /// Apply the documented threshold.
    fn default() -> Self {
        Self { max_lines: 400 }
    }
}

/// Configure the maximum number of branches in a conditional.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConditionalMaxBranches {
    /// Maximum number of branches.
    pub max_branches: usize,
}
impl Default for ConditionalMaxBranches {
    /// Apply the documented threshold.
    fn default() -> Self {
        Self { max_branches: 2 }
    }
}

/// Configure extra test markers recognized by a lint.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TestAttributes {
    /// Additional attribute names.
    pub additional_test_attributes: Vec<String>,
}

/// Configure the panic-in-main exception.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct NoUnwrapOrElsePanic {
    /// Permit the operation in `main`.
    pub allow_in_main: bool,
}

/// Configure fixture extraction suggestions.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RstestHelper {
    /// Minimum number of calls.
    pub min_calls: usize,
    /// Minimum number of distinct tests.
    pub min_distinct_tests: usize,
    /// Require matching fixture argument names.
    pub require_identical_fixture_arg_names: bool,
    /// Attributes treated as data providers.
    pub provider_param_attributes: Vec<String>,
    /// Permit the source-callee fallback.
    pub use_source_callee_fallback: bool,
}
impl Default for RstestHelper {
    /// Apply the documented fixture defaults.
    fn default() -> Self {
        Self {
            min_calls: 2,
            min_distinct_tests: 2,
            require_identical_fixture_arg_names: false,
            provider_param_attributes: ["case", "values", "files", "future", "context"]
                .into_iter()
                .map(String::from)
                .collect(),
            use_source_callee_fallback: false,
        }
    }
}

/// Configure the capability-oriented filesystem lint.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct NoStdFs {
    /// Crates exempt from the lint.
    pub excluded_crates: Vec<String>,
    /// Module paths exempt from the lint.
    pub excluded_paths: Vec<String>,
}

/// Configure the bumpy-road complexity lint.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BumpyRoad {
    /// Threshold for a complexity bump.
    pub threshold: f64,
    /// Smoothing window.
    pub window: usize,
    /// Minimum number of lines in a bump.
    pub min_bump_lines: usize,
    /// Include closure bodies in analysis.
    pub include_closures: bool,
    /// Contributions of the individual complexity factors.
    pub weights: BumpyWeights,
}
impl Default for BumpyRoad {
    /// Apply the documented complexity defaults.
    fn default() -> Self {
        Self {
            threshold: 2.5,
            window: 3,
            min_bump_lines: 2,
            include_closures: false,
            weights: BumpyWeights::default(),
        }
    }
}

/// Configure weights for bumpy-road complexity factors.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BumpyWeights {
    /// Contribution of nesting depth.
    pub depth: f64,
    /// Contribution of predicates.
    pub predicate: f64,
    /// Contribution of control flow.
    pub flow: f64,
}
impl Default for BumpyWeights {
    /// Apply the lint's documented weights.
    fn default() -> Self {
        Self {
            depth: 1.0,
            predicate: 0.5,
            flow: 0.5,
        }
    }
}
