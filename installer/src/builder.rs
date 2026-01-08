//! Cargo build orchestration for lint crates.
//!
//! This module provides utilities to build Dylint lint crates in release mode
//! with the required features enabled.

use crate::error::{InstallerError, Result};
use crate::toolchain::Toolchain;
use camino::Utf8PathBuf;
use std::process::Command;

// Re-export from submodules for backwards compatibility
pub use crate::crate_name::CrateName;
pub use crate::resolution::{
    CrateResolutionOptions, EXPERIMENTAL_LINT_CRATES, LINT_CRATES, SUITE_CRATE, is_known_crate,
    resolve_crates, validate_crate_names,
};
pub use crate::workspace::find_workspace_root;

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

/// Trait for building lint crates, enabling dependency injection for tests.
///
/// This trait abstracts the build operation so that tests can mock the builder
/// and verify that `perform_build` constructs the correct configuration without
/// actually invoking cargo.
#[cfg_attr(test, mockall::automock)]
pub trait CrateBuilder {
    /// Build all specified crates.
    ///
    /// # Errors
    ///
    /// Returns an error if any crate fails to build.
    fn build_all(&self, crates: &[CrateName]) -> Result<Vec<BuildResult>>;
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

    /// Returns a reference to the build configuration.
    ///
    /// This method is primarily useful for testing to verify that the correct
    /// configuration was constructed.
    #[must_use]
    pub fn config(&self) -> &BuildConfig {
        &self.config
    }
}

impl CrateBuilder for Builder {
    fn build_all(&self, crates: &[CrateName]) -> Result<Vec<BuildResult>> {
        self.build_all(crates)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Create a test builder with the given experimental flag.
    fn test_builder(experimental: bool) -> Builder {
        Builder {
            config: BuildConfig {
                toolchain: Toolchain::with_override(
                    &Utf8PathBuf::from("/tmp/test"),
                    "nightly-2025-09-18",
                ),
                target_dir: Utf8PathBuf::from("/tmp/target"),
                jobs: None,
                verbosity: 0,
                experimental,
            },
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

    #[rstest]
    #[case::non_suite_crate("module_max_lines", false, "dylint-driver")]
    #[case::non_suite_with_experimental("module_max_lines", true, "dylint-driver")]
    #[case::suite_without_experimental("suite", false, "dylint-driver")]
    fn features_for_crate_returns_expected_features(
        #[case] crate_name: &str,
        #[case] experimental: bool,
        #[case] expected: &str,
    ) {
        let builder = test_builder(experimental);
        let result = builder.features_for_crate(&CrateName::from(crate_name));
        assert_eq!(result, expected);
    }

    #[test]
    fn features_for_crate_includes_experimental_for_suite() {
        let builder = test_builder(true);
        let result = builder.features_for_crate(&CrateName::from("suite"));

        // Should start with dylint-driver
        assert!(result.starts_with("dylint-driver"));

        // Should include experimental features
        assert!(result.contains("experimental-"));
        // Verify format: experimental-{lint-name-with-hyphens}
        for lint in EXPERIMENTAL_LINT_CRATES {
            let expected_feature = format!("experimental-{}", lint.replace('_', "-"));
            assert!(
                result.contains(&expected_feature),
                "expected {expected_feature} in {result}"
            );
        }
    }

    #[test]
    fn experimental_features_derives_from_experimental_lint_crates() {
        let features = Builder::experimental_features();

        // Verify comma-separated format
        let parts: Vec<_> = features.split(',').collect();
        assert_eq!(parts.len(), EXPERIMENTAL_LINT_CRATES.len());

        // Verify each feature follows the expected pattern
        for (i, lint) in EXPERIMENTAL_LINT_CRATES.iter().enumerate() {
            let expected = format!("experimental-{}", lint.replace('_', "-"));
            assert_eq!(parts[i], expected);
        }
    }
}
