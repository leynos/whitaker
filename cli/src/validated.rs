//! Convert merged configuration data into validated application settings.
//!
//! The lint crates normalize out-of-range tuning values rather than rejecting
//! them, so applying the same policy here keeps the package boundary in step
//! with the domain. Presentation values that have no normalization policy are
//! rejected with a typed error instead.

use ortho_config::OrthoResult;

use crate::{
    error::invalid,
    lint_config::{
        BumpyRoad, BumpyWeights, ConditionalMaxBranches, ModuleMaxLines, NoStdFs,
        NoUnwrapOrElsePanic, RstestHelper, TestAttributes,
    },
    settings::{Configuration, Diagnostics, Install, Lint, Output},
};

/// Settings that have passed application validation and domain normalization.
#[derive(Clone, Debug)]
pub struct ValidatedSettings {
    /// Select the lint bundles to run.
    pub lint: Lint,
    /// Control installation policy.
    pub install: Install,
    /// Select diagnostic presentation.
    pub diagnostics: Diagnostics,
    /// Select output presentation.
    pub output: Output,
    /// Module line-count policy.
    pub module_max_lines: ModuleMaxLines,
    /// Conditional branch-count policy.
    pub conditional_max_n_branches: ConditionalMaxBranches,
    /// Additional test attributes for the expect lint.
    pub no_expect_outside_tests: TestAttributes,
    /// Additional test attributes for the example lint.
    pub test_must_not_have_example: TestAttributes,
    /// Panic-in-main policy.
    pub no_unwrap_or_else_panic: NoUnwrapOrElsePanic,
    /// Fixture extraction policy.
    pub rstest_helper_should_be_fixture: RstestHelper,
    /// Filesystem exclusion policy.
    pub no_std_fs_operations: NoStdFs,
    /// Complexity scoring policy.
    pub bumpy_road_function: BumpyRoad,
    /// Deferred command-specific configuration namespaces.
    pub cmds: std::collections::BTreeMap<String, serde_json::Value>,
}

impl ValidatedSettings {
    /// Validate presentation values and normalize domain tuning settings.
    ///
    /// # Errors
    ///
    /// Returns a configuration error when an output policy is not one of the
    /// documented values.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_cli::{Configuration, ValidatedSettings};
    /// let settings = ValidatedSettings::from_configuration(Configuration::default())?;
    /// assert_eq!(settings.output.colour, "auto");
    /// # Ok::<(), std::sync::Arc<ortho_config::OrthoError>>(())
    /// ```
    pub fn from_configuration(configuration: Configuration) -> OrthoResult<Self> {
        validate_output(&configuration.output)?;
        Ok(normalize(configuration))
    }
}

/// Reject an unrecognized colour or progress policy.
fn validate_output(output: &Output) -> OrthoResult<()> {
    if !matches!(output.colour.as_str(), "auto" | "always" | "never") {
        return Err(invalid("output.colour", "expected auto, always, or never"));
    }
    if !matches!(output.progress.as_str(), "auto" | "plain" | "off") {
        return Err(invalid("output.progress", "expected auto, plain, or off"));
    }
    Ok(())
}

/// Apply every domain normalization rule to the merged settings.
fn normalize(mut configuration: Configuration) -> ValidatedSettings {
    normalize_bumpy_road(&mut configuration.bumpy_road_function);
    configuration.conditional_max_n_branches.max_branches =
        configuration.conditional_max_n_branches.max_branches.max(1);
    normalize_rstest_helper(&mut configuration.rstest_helper_should_be_fixture);
    ValidatedSettings {
        lint: configuration.lint,
        install: configuration.install,
        diagnostics: configuration.diagnostics,
        output: configuration.output,
        module_max_lines: configuration.module_max_lines,
        conditional_max_n_branches: configuration.conditional_max_n_branches,
        no_expect_outside_tests: configuration.no_expect_outside_tests,
        test_must_not_have_example: configuration.test_must_not_have_example,
        no_unwrap_or_else_panic: configuration.no_unwrap_or_else_panic,
        rstest_helper_should_be_fixture: configuration.rstest_helper_should_be_fixture,
        no_std_fs_operations: configuration.no_std_fs_operations,
        bumpy_road_function: configuration.bumpy_road_function,
        cmds: configuration.cmds,
    }
}

/// Clamp bumpy-road values using the lint's normalization policy.
fn normalize_bumpy_road(bumpy: &mut BumpyRoad) {
    let defaults = BumpyRoad::default();
    bumpy.threshold = finite_non_negative(bumpy.threshold, defaults.threshold);
    bumpy.window = if is_valid_window(bumpy.window) {
        bumpy.window
    } else {
        defaults.window
    };
    bumpy.min_bump_lines = bumpy.min_bump_lines.max(1);
    bumpy.weights = BumpyWeights {
        depth: finite_non_negative(bumpy.weights.depth, defaults.weights.depth),
        predicate: finite_non_negative(bumpy.weights.predicate, defaults.weights.predicate),
        flow: finite_non_negative(bumpy.weights.flow, defaults.weights.flow),
    };
}

/// Keep a finite, non-negative candidate or fall back to its default.
fn finite_non_negative(candidate: f64, fallback: f64) -> f64 {
    if candidate.is_finite() && candidate >= 0.0 {
        candidate
    } else {
        fallback
    }
}

/// Accept only non-zero odd windows, as the smoothing pass requires.
fn is_valid_window(window: usize) -> bool {
    window != 0 && (window & 1) == 1
}

/// Clamp fixture thresholds and canonicalize provider attributes.
fn normalize_rstest_helper(helper: &mut RstestHelper) {
    helper.min_calls = helper.min_calls.max(2);
    helper.min_distinct_tests = helper.min_distinct_tests.max(2);
    helper.provider_param_attributes =
        normalize_provider_attributes(std::mem::take(&mut helper.provider_param_attributes));
}

/// Remove surrounding whitespace and every leading `rstest::` prefix.
///
/// Each prefix may expose further whitespace or another prefix, so the two
/// steps repeat until the name is stable.
fn canonical_attribute(attribute: &str) -> &str {
    let mut canonical = attribute.trim();
    while let Some(stripped) = canonical.strip_prefix("rstest::") {
        canonical = stripped.trim();
    }
    canonical
}

/// Trim, de-duplicate, and prefix-free provider attribute names.
fn normalize_provider_attributes(attributes: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for attribute in attributes
        .into_iter()
        .map(|attribute| canonical_attribute(&attribute).to_owned())
        .filter(|attribute| !attribute.is_empty())
    {
        if !normalized.contains(&attribute) {
            normalized.push(attribute);
        }
    }
    if normalized.is_empty() {
        return RstestHelper::default().provider_param_attributes;
    }
    normalized
}

#[cfg(test)]
#[path = "validated_tests.rs"]
mod tests;
