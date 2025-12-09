//! Cargo build orchestration for lint crates.
//!
//! This module provides utilities to build Dylint lint crates in release mode
//! with the required features enabled.

use crate::error::{InstallerError, Result};
use crate::toolchain::Toolchain;
use camino::{Utf8Path, Utf8PathBuf};
use std::process::Command;

/// Static list of lint crates available for building.
///
/// This list includes all individual lint crates plus the aggregated suite.
pub const LINT_CRATES: &[&str] = &[
    "conditional_max_n_branches",
    "function_attrs_follow_docs",
    "module_max_lines",
    "module_must_have_inner_docs",
    "no_expect_outside_tests",
    "no_std_fs_operations",
    "no_unwrap_or_else_panic",
];

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
    /// Whether to print verbose output.
    pub verbose: bool,
}

/// Result of building a single crate.
#[derive(Debug, Clone)]
pub struct BuildResult {
    /// Name of the crate that was built.
    pub crate_name: String,
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
    pub fn build_crate(&self, crate_name: &str) -> Result<BuildResult> {
        let mut cmd = Command::new("cargo");

        cmd.arg(format!("+{}", self.config.toolchain.channel()));
        cmd.args(["build", "--release", "--features", "dylint-driver"]);
        cmd.args(["-p", crate_name]);

        if let Some(jobs) = self.config.jobs {
            cmd.args(["-j", &jobs.to_string()]);
        }

        cmd.env("CARGO_TARGET_DIR", self.config.target_dir.as_str());
        cmd.current_dir(self.config.toolchain.workspace_root());

        if self.config.verbose {
            cmd.arg("-v");
        }

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(InstallerError::BuildFailed {
                crate_name: crate_name.to_owned(),
                reason: stderr.to_string(),
            });
        }

        let library_path = self.library_path(crate_name);

        Ok(BuildResult {
            crate_name: crate_name.to_owned(),
            library_path,
        })
    }

    /// Build all specified crates.
    ///
    /// # Errors
    ///
    /// Returns an error if any crate fails to build.
    pub fn build_all(&self, crates: &[&str]) -> Result<Vec<BuildResult>> {
        let mut results = Vec::with_capacity(crates.len());

        for crate_name in crates {
            let result = self.build_crate(crate_name)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Compute the expected library path for a crate.
    fn library_path(&self, crate_name: &str) -> Utf8PathBuf {
        let lib_name = format!(
            "{}{}{}",
            library_prefix(),
            crate_name.replace('-', "_"),
            library_extension()
        );

        self.config.target_dir.join("release").join(lib_name)
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

/// Validate that all specified crate names are known lint crates.
///
/// # Errors
///
/// Returns an error if any crate name is not recognised.
pub fn validate_crate_names(names: &[String]) -> Result<()> {
    for name in names {
        let is_valid = LINT_CRATES.contains(&name.as_str()) || name == SUITE_CRATE;
        if !is_valid {
            return Err(InstallerError::LintCrateNotFound { name: name.clone() });
        }
    }
    Ok(())
}

/// Build the list of crates to compile based on CLI options.
#[must_use]
pub fn resolve_crates(
    specific_lints: &[String],
    suite_only: bool,
    no_suite: bool,
) -> Vec<&'static str> {
    if suite_only {
        return vec![SUITE_CRATE];
    }

    if !specific_lints.is_empty() {
        return specific_lints
            .iter()
            .filter_map(|name| {
                LINT_CRATES
                    .iter()
                    .find(|&&c| c == name)
                    .copied()
                    .or_else(|| (name == SUITE_CRATE).then_some(SUITE_CRATE))
            })
            .collect();
    }

    let mut crates: Vec<&str> = LINT_CRATES.to_vec();
    if !no_suite {
        crates.push(SUITE_CRATE);
    }
    crates
}

/// Find the workspace root by looking for `Cargo.toml` with `[workspace]`.
///
/// # Errors
///
/// Returns an error if the workspace root cannot be determined.
pub fn find_workspace_root(start: &Utf8Path) -> Result<Utf8PathBuf> {
    let mut current = start.to_owned();

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            let contents = std::fs::read_to_string(&cargo_toml)?;
            if contents.contains("[workspace]") {
                return Ok(current);
            }
        }

        match current.parent() {
            Some(parent) => current = parent.to_owned(),
            None => break,
        }
    }

    Err(InstallerError::ToolchainDetection {
        reason: "could not find workspace root".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_crates_returns_all_by_default() {
        let crates = resolve_crates(&[], false, false);
        assert!(crates.contains(&"module_max_lines"));
        assert!(crates.contains(&SUITE_CRATE));
    }

    #[test]
    fn resolve_crates_suite_only() {
        let crates = resolve_crates(&[], true, false);
        assert_eq!(crates, vec![SUITE_CRATE]);
    }

    #[test]
    fn resolve_crates_no_suite() {
        let crates = resolve_crates(&[], false, true);
        assert!(!crates.contains(&SUITE_CRATE));
        assert!(crates.contains(&"module_max_lines"));
    }

    #[test]
    fn resolve_crates_specific_lints() {
        let specific = vec!["module_max_lines".to_owned()];
        let crates = resolve_crates(&specific, false, false);
        assert_eq!(crates, vec!["module_max_lines"]);
    }

    #[test]
    fn validate_known_crates_succeeds() {
        let names = vec!["module_max_lines".to_owned(), "suite".to_owned()];
        assert!(validate_crate_names(&names).is_ok());
    }

    #[test]
    fn validate_unknown_crate_fails() {
        let names = vec!["nonexistent_lint".to_owned()];
        let result = validate_crate_names(&names);
        assert!(result.is_err());
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
    }
}
