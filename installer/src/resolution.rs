//! Crate resolution and validation.
//!
//! This module determines which lint crates to build based on CLI options and
//! validates that requested crate names are known.

use crate::crate_name::CrateName;
use crate::error::{InstallerError, Result};
use log::debug;

/// Static list of lint crates available for building.
///
/// This list includes all individual lint crates. The aggregated suite is
/// defined separately as [`SUITE_CRATE`].
pub const LINT_CRATES: &[&str] = &[
    "bumpy_road_function",
    "conditional_max_n_branches",
    "function_attrs_follow_docs",
    "module_max_lines",
    "module_must_have_inner_docs",
    "no_expect_outside_tests",
    "test_must_not_have_example",
    "no_std_fs_operations",
    "no_unwrap_or_else_panic",
];

/// Static list of experimental lint crates.
///
/// These lints are not included in the default suite but can be enabled via
/// the `--experimental` flag.
pub const EXPERIMENTAL_LINT_CRATES: &[&str] = &["rstest_helper_should_be_fixture"];

/// The aggregated suite crate name.
pub const SUITE_CRATE: &str = "whitaker_suite";

/// Options controlling crate resolution behaviour.
#[derive(Debug, Clone, Default)]
pub struct CrateResolutionOptions {
    /// Build all individual lint crates instead of the aggregated suite.
    pub individual_lints: bool,
    /// Include experimental lint crates when `individual_lints` is true.
    pub experimental: bool,
}

/// Build the list of crates to compile based on CLI options.
///
/// By default, only the aggregated suite is built. Use `individual_lints` to
/// build all individual lint crates instead, or provide `specific_lints` to
/// cherry-pick particular lints.
///
/// The `experimental` flag has different effects depending on the mode:
/// - In `individual_lints` mode, experimental lint crates from
///   `EXPERIMENTAL_LINT_CRATES` are included in the returned crate list.
/// - In suite mode (default), the `experimental` flag is used by `BuildConfig`
///   to enable experimental features when building the suite crate.
/// - When `specific_lints` are provided, the returned list is exactly the
///   requested crate list after validation. Experimental crates still require
///   the `experimental` flag during validation.
///
/// Note: This function assumes that `specific_lints` have been validated via
/// `validate_crate_names()` prior to calling. Callers must validate inputs
/// first to get proper error messages for unknown names.
#[must_use]
pub fn resolve_crates(
    specific_lints: &[CrateName],
    options: &CrateResolutionOptions,
) -> Vec<CrateName> {
    if !specific_lints.is_empty() {
        // Assumes names have been validated via validate_crate_names().
        debug!(
            target: "whitaker_installer::resolution",
            "using explicit lint crate selection: {:?}",
            specific_lints
        );
        return specific_lints.to_vec();
    }

    if options.individual_lints {
        let mut crates: Vec<CrateName> = LINT_CRATES.iter().map(|&c| CrateName::from(c)).collect();
        if options.experimental {
            debug!(
                target: "whitaker_installer::resolution",
                "including experimental lint crates because --experimental is enabled"
            );
            crates.extend(EXPERIMENTAL_LINT_CRATES.iter().map(|&c| CrateName::from(c)));
        } else {
            debug!(
                target: "whitaker_installer::resolution",
                "excluding experimental lint crates because --experimental is disabled"
            );
        }
        return crates;
    }

    // Default: suite only (experimental feature is handled by BuildConfig)
    debug!(
        target: "whitaker_installer::resolution",
        "using suite crate selection; experimental feature handling is deferred to build config"
    );
    vec![CrateName::from(SUITE_CRATE)]
}

/// Check whether a crate name is a known lint crate or the suite.
#[must_use]
pub fn is_known_crate(name: &CrateName) -> bool {
    let s = name.as_str();
    LINT_CRATES.contains(&s) || EXPERIMENTAL_LINT_CRATES.contains(&s) || s == SUITE_CRATE
}

/// Check whether a crate name identifies an experimental lint crate.
#[must_use]
pub fn is_experimental_crate(name: &CrateName) -> bool {
    EXPERIMENTAL_LINT_CRATES.contains(&name.as_str())
}

/// Validate that all specified crate names are known lint crates.
///
/// # Errors
///
/// Returns an error if any crate name is not recognised, or if an experimental
/// crate is requested without `--experimental`.
pub fn validate_crate_names(names: &[CrateName], options: &CrateResolutionOptions) -> Result<()> {
    for name in names {
        if !is_known_crate(name) {
            debug!(
                target: "whitaker_installer::resolution",
                "rejecting unknown lint crate `{}`",
                name.as_str()
            );
            return Err(InstallerError::LintCrateNotFound { name: name.clone() });
        }
        if is_experimental_crate(name) && !options.experimental {
            debug!(
                target: "whitaker_installer::resolution",
                "rejecting experimental lint crate `{}` because --experimental is disabled",
                name.as_str()
            );
            return Err(InstallerError::ExperimentalLintRequiresFlag { name: name.clone() });
        }
        debug!(
            target: "whitaker_installer::resolution",
            "accepted lint crate `{}` for resolution",
            name.as_str()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Test configuration for resolve_crates variants.
    struct ResolveCratesCase {
        individual_lints: bool,
        experimental: bool,
        expect_lint: bool,
        expect_suite: bool,
        expect_bumpy_road: bool,
        expect_experimental_lint: bool,
    }

    /// Parameterised tests for resolve_crates variants.
    #[rstest]
    #[case::default_suite_only(ResolveCratesCase { individual_lints: false, experimental: false, expect_lint: false, expect_suite: true, expect_bumpy_road: false, expect_experimental_lint: false })]
    #[case::individual_lints(ResolveCratesCase { individual_lints: true, experimental: false, expect_lint: true, expect_suite: false, expect_bumpy_road: true, expect_experimental_lint: false })]
    #[case::individual_with_experimental(ResolveCratesCase { individual_lints: true, experimental: true, expect_lint: true, expect_suite: false, expect_bumpy_road: true, expect_experimental_lint: true })]
    #[case::suite_with_experimental(ResolveCratesCase { individual_lints: false, experimental: true, expect_lint: false, expect_suite: true, expect_bumpy_road: false, expect_experimental_lint: false })]
    fn resolve_crates_variants(#[case] case: ResolveCratesCase) {
        let options = CrateResolutionOptions {
            individual_lints: case.individual_lints,
            experimental: case.experimental,
        };
        let crates = resolve_crates(&[], &options);

        assert_eq!(
            crates.contains(&CrateName::from("module_max_lines")),
            case.expect_lint,
            "lint crate inclusion mismatch"
        );
        assert_eq!(
            crates.contains(&CrateName::from(SUITE_CRATE)),
            case.expect_suite,
            "suite crate inclusion mismatch"
        );
        assert_eq!(
            crates.contains(&CrateName::from("bumpy_road_function")),
            case.expect_bumpy_road,
            "bumpy_road_function inclusion mismatch"
        );
        assert_eq!(
            crates.contains(&CrateName::from("rstest_helper_should_be_fixture")),
            case.expect_experimental_lint,
            "rstest_helper_should_be_fixture inclusion mismatch"
        );
    }

    #[test]
    fn resolve_crates_specific_lints() {
        let specific = vec![CrateName::from("module_max_lines")];
        let crates = resolve_crates(&specific, &CrateResolutionOptions::default());
        assert_eq!(crates, vec![CrateName::from("module_max_lines")]);
    }

    #[rstest]
    #[case::valid(&["module_max_lines", "whitaker_suite"], false, true)]
    #[case::bumpy_road_function(&["bumpy_road_function"], false, true)]
    #[case::experimental_with_flag(&["rstest_helper_should_be_fixture"], true, true)]
    #[case::experimental_without_flag(&["rstest_helper_should_be_fixture"], false, false)]
    #[case::unknown(&["nonexistent_lint"], false, false)]
    fn validate_crate_names_variants(
        #[case] names: &[&str],
        #[case] experimental: bool,
        #[case] expect_ok: bool,
    ) {
        let crate_names: Vec<CrateName> = names.iter().map(|&s| CrateName::from(s)).collect();
        let options = CrateResolutionOptions {
            experimental,
            ..CrateResolutionOptions::default()
        };
        let res = validate_crate_names(&crate_names, &options);
        if expect_ok {
            assert!(res.is_ok());
        } else {
            let err = res.expect_err("expected validation failure");
            assert!(
                matches!(
                    &err,
                    InstallerError::LintCrateNotFound { name }
                        | InstallerError::ExperimentalLintRequiresFlag { name }
                        if *name == crate_names[0]
                ),
                "unexpected error: {err:?}"
            );
        }
    }

    #[test]
    fn validate_crate_names_returns_experimental_lint_requires_flag_error() {
        let name = CrateName::from("rstest_helper_should_be_fixture");
        let options = CrateResolutionOptions {
            experimental: false,
            ..CrateResolutionOptions::default()
        };

        let error = validate_crate_names(std::slice::from_ref(&name), &options)
            .expect_err("experimental lint should require explicit opt-in");

        assert!(
            matches!(&error, InstallerError::ExperimentalLintRequiresFlag { name: error_name } if *error_name == name),
            "expected ExperimentalLintRequiresFlag, got {error:?}"
        );
    }
}
