//! Cargo build orchestration for lint crates.
//!
//! This module provides utilities to build Dylint lint crates in release mode
//! with the required features enabled.

use crate::error::{InstallerError, Result};
use crate::toolchain::Toolchain;
use camino::{Utf8Path, Utf8PathBuf};
use std::fmt;
use std::process::Command;

/// A semantic crate name for lint libraries.
///
/// This newtype wrapper provides type safety for crate names, ensuring they are
/// passed explicitly rather than as raw strings. Validation is performed by
/// [`validate_crate_names`] and related helpers, not by this type itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrateName(String);

impl CrateName {
    /// Create a new crate name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the crate name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for CrateName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CrateName {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for CrateName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Static list of lint crates available for building.
///
/// This list includes all individual lint crates. The aggregated suite is
/// defined separately as [`SUITE_CRATE`].
pub const LINT_CRATES: &[&str] = &[
    "conditional_max_n_branches",
    "function_attrs_follow_docs",
    "module_max_lines",
    "module_must_have_inner_docs",
    "no_expect_outside_tests",
    "no_std_fs_operations",
    "no_unwrap_or_else_panic",
];

/// Static list of experimental lint crates.
///
/// These lints are not included in the default suite but can be enabled via
/// the `--experimental` flag.
pub const EXPERIMENTAL_LINT_CRATES: &[&str] = &["bumpy_road_function"];

/// The aggregated suite crate name.
pub const SUITE_CRATE: &str = "suite";

/// Configuration for the build process.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// The Rust toolchain to use.
    pub toolchain: Toolchain,
    /// Directory for build artifacts.
    pub target_dir: Utf8PathBuf,
    /// Number of parallel build jobs (None for cargo default).
    pub jobs: Option<usize>,
    /// Verbosity level for cargo output.
    pub verbosity: u8,
    /// Include experimental lints when building the suite.
    pub experimental: bool,
}

/// Result of building a single crate.
#[derive(Debug, Clone)]
pub struct BuildResult {
    /// Name of the crate that was built.
    pub crate_name: CrateName,
    /// Path to the compiled library.
    pub library_path: Utf8PathBuf,
}

/// Builder for compiling lint crates.
pub struct Builder {
    config: BuildConfig,
}

impl Builder {
    /// Create a new builder with the given configuration.
    #[must_use]
    pub fn new(config: BuildConfig) -> Self {
        Self { config }
    }

    /// Build a single lint crate.
    ///
    /// # Errors
    ///
    /// Returns an error if the cargo build command fails.
    pub fn build_crate(&self, crate_name: &CrateName) -> Result<BuildResult> {
        let mut cmd = Command::new("cargo");

        cmd.arg(format!("+{}", self.config.toolchain.channel()));
        cmd.args(["build", "--release"]);

        let features = self.features_for_crate(crate_name);
        cmd.args(["--features", &features]);

        cmd.args(["-p", crate_name.as_str()]);

        if let Some(jobs) = self.config.jobs {
            cmd.args(["-j", &jobs.to_string()]);
        }

        cmd.env("CARGO_TARGET_DIR", self.config.target_dir.as_str());
        cmd.current_dir(self.config.toolchain.workspace_root());

        for _ in 0..self.config.verbosity {
            cmd.arg("-v");
        }

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(InstallerError::BuildFailed {
                crate_name: crate_name.clone(),
                reason: stderr.to_string(),
            });
        }

        let library_path = self.library_path(crate_name);
        if !library_path.exists() {
            return Err(InstallerError::BuildFailed {
                crate_name: crate_name.clone(),
                reason: format!(
                    "cargo succeeded but expected library was not found at: {library_path}"
                ),
            });
        }

        Ok(BuildResult {
            crate_name: crate_name.clone(),
            library_path,
        })
    }

    /// Build all specified crates.
    ///
    /// # Errors
    ///
    /// Returns an error if any crate fails to build.
    pub fn build_all(&self, crates: &[CrateName]) -> Result<Vec<BuildResult>> {
        let mut results = Vec::with_capacity(crates.len());

        for crate_name in crates {
            let result = self.build_crate(crate_name)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Compute the expected library path for a crate.
    fn library_path(&self, crate_name: &CrateName) -> Utf8PathBuf {
        let lib_name = format!(
            "{}{}{}",
            library_prefix(),
            crate_name.as_str().replace('-', "_"),
            library_extension()
        );

        self.config.target_dir.join("release").join(lib_name)
    }

    /// Determine which features to enable for a given crate.
    ///
    /// For the suite crate, this includes the experimental feature when enabled.
    /// For individual lint crates, only the `dylint-driver` feature is needed.
    fn features_for_crate(&self, crate_name: &CrateName) -> String {
        if crate_name.as_str() == SUITE_CRATE && self.config.experimental {
            let experimental = Self::experimental_features();
            if experimental.is_empty() {
                "dylint-driver".to_owned()
            } else {
                format!("dylint-driver,{experimental}")
            }
        } else {
            "dylint-driver".to_owned()
        }
    }

    /// Generate the comma-separated list of experimental feature flags.
    ///
    /// Feature names follow the pattern `experimental-{lint_name_with_hyphens}`,
    /// derived from `EXPERIMENTAL_LINT_CRATES` to keep the source of truth in one
    /// place.
    ///
    /// Returns an empty string if `EXPERIMENTAL_LINT_CRATES` is empty.
    fn experimental_features() -> String {
        EXPERIMENTAL_LINT_CRATES
            .iter()
            .map(|&name| format!("experimental-{}", name.replace('_', "-")))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Return the platform-specific library file extension (including the dot).
#[must_use]
pub const fn library_extension() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        ".dylib"
    }
    #[cfg(target_os = "windows")]
    {
        ".dll"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        ".so"
    }
}

/// Return the platform-specific library filename prefix.
#[must_use]
pub const fn library_prefix() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        ""
    }
    #[cfg(not(target_os = "windows"))]
    {
        "lib"
    }
}

/// Check whether a crate name is a known lint crate or the suite.
#[must_use]
pub fn is_known_crate(name: &CrateName) -> bool {
    let s = name.as_str();
    LINT_CRATES.contains(&s) || EXPERIMENTAL_LINT_CRATES.contains(&s) || s == SUITE_CRATE
}

/// Validate that all specified crate names are known lint crates.
///
/// # Errors
///
/// Returns an error if any crate name is not recognised.
pub fn validate_crate_names(names: &[CrateName]) -> Result<()> {
    for name in names {
        if !is_known_crate(name) {
            return Err(InstallerError::LintCrateNotFound { name: name.clone() });
        }
    }
    Ok(())
}

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
/// - When `specific_lints` are provided, experimental crates are accepted if
///   present, but the `experimental` flag does not affect the returned list.
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
        return specific_lints.to_vec();
    }

    if options.individual_lints {
        let mut crates: Vec<CrateName> = LINT_CRATES.iter().map(|&c| CrateName::from(c)).collect();
        if options.experimental {
            crates.extend(EXPERIMENTAL_LINT_CRATES.iter().map(|&c| CrateName::from(c)));
        }
        return crates;
    }

    // Default: suite only (experimental feature is handled by BuildConfig)
    vec![CrateName::from(SUITE_CRATE)]
}

/// Find the workspace root by looking for `Cargo.toml` with `[workspace]`.
///
/// # Errors
///
/// Returns an error if the workspace root cannot be determined, or if a
/// `Cargo.toml` file cannot be read or parsed.
pub fn find_workspace_root(start: &Utf8Path) -> Result<Utf8PathBuf> {
    let mut current = start.to_owned();

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() && is_workspace_root(&cargo_toml)? {
            return Ok(current);
        }

        match current.parent() {
            Some(parent) => current = parent.to_owned(),
            None => break,
        }
    }

    Err(InstallerError::WorkspaceNotFound {
        reason: "could not find Cargo.toml with [workspace] section".to_owned(),
    })
}

/// Check if a `Cargo.toml` file contains a `[workspace]` section.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
fn is_workspace_root(cargo_toml: &Utf8Path) -> Result<bool> {
    let contents = std::fs::read_to_string(cargo_toml)?;
    let table = contents
        .parse::<toml::Table>()
        .map_err(|e| InstallerError::InvalidCargoToml {
            path: cargo_toml.to_owned(),
            reason: e.to_string(),
        })?;
    Ok(table.contains_key("workspace"))
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
        expect_experimental: bool,
    }

    /// Parameterised tests for resolve_crates variants.
    #[rstest]
    #[case::default_suite_only(ResolveCratesCase { individual_lints: false, experimental: false, expect_lint: false, expect_suite: true, expect_experimental: false })]
    #[case::individual_lints(ResolveCratesCase { individual_lints: true, experimental: false, expect_lint: true, expect_suite: false, expect_experimental: false })]
    #[case::individual_with_experimental(ResolveCratesCase { individual_lints: true, experimental: true, expect_lint: true, expect_suite: false, expect_experimental: true })]
    #[case::suite_with_experimental(ResolveCratesCase { individual_lints: false, experimental: true, expect_lint: false, expect_suite: true, expect_experimental: false })]
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
            case.expect_experimental,
            "experimental crate inclusion mismatch"
        );
    }

    #[test]
    fn resolve_crates_specific_lints() {
        let specific = vec![CrateName::from("module_max_lines")];
        let crates = resolve_crates(&specific, &CrateResolutionOptions::default());
        assert_eq!(crates, vec![CrateName::from("module_max_lines")]);
    }

    #[rstest]
    #[case::valid(&["module_max_lines", "suite"], true)]
    #[case::experimental_lint(&["bumpy_road_function"], true)]
    #[case::unknown(&["nonexistent_lint"], false)]
    fn validate_crate_names_variants(#[case] names: &[&str], #[case] expect_ok: bool) {
        let crate_names: Vec<CrateName> = names.iter().map(|&s| CrateName::from(s)).collect();
        let res = validate_crate_names(&crate_names);
        if expect_ok {
            assert!(res.is_ok());
        } else {
            let err = res.expect_err("expected validation failure");
            assert!(
                matches!(&err, InstallerError::LintCrateNotFound { name } if *name == crate_names[0]),
                "unexpected error: {err:?}"
            );
        }
    }

    #[test]
    fn library_extension_is_correct() {
        let ext = library_extension();
        #[cfg(target_os = "linux")]
        assert_eq!(ext, ".so");
        #[cfg(target_os = "macos")]
        assert_eq!(ext, ".dylib");
        #[cfg(target_os = "windows")]
        assert_eq!(ext, ".dll");
        // Fallback for other Unix-like platforms (e.g., FreeBSD, OpenBSD)
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        assert_eq!(ext, ".so");
    }
}
