//! Cargo build orchestration for lint crates.
//!
//! This module provides utilities to build Dylint lint crates in release mode
//! with the required features enabled.

use crate::error::{InstallerError, Result};
use crate::toolchain::Toolchain;
use camino::{Utf8Path, Utf8PathBuf};
use std::fmt;
use std::process::Command;

/// A validated crate name for lint libraries.
///
/// This newtype wrapper provides type safety for crate names, ensuring they are
/// passed explicitly rather than as raw strings.
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
        cmd.args(["build", "--release", "--features", "dylint-driver"]);
        cmd.args(["-p", crate_name.as_str()]);

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
                crate_name: crate_name.clone(),
                reason: stderr.to_string(),
            });
        }

        let library_path = self.library_path(crate_name);

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
    LINT_CRATES.contains(&s) || s == SUITE_CRATE
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

/// Build the list of crates to compile based on CLI options.
///
/// Note: This function assumes that `specific_lints` have been validated via
/// `validate_crate_names()` prior to calling. Callers must validate inputs
/// first to get proper error messages for unknown names.
#[must_use]
pub fn resolve_crates(
    specific_lints: &[CrateName],
    suite_only: bool,
    no_suite: bool,
) -> Vec<CrateName> {
    if suite_only {
        return vec![CrateName::from(SUITE_CRATE)];
    }

    if !specific_lints.is_empty() {
        // Assumes names have been validated via validate_crate_names().
        return specific_lints.to_vec();
    }

    let mut crates: Vec<CrateName> = LINT_CRATES.iter().map(|&c| CrateName::from(c)).collect();
    if !no_suite {
        crates.push(CrateName::from(SUITE_CRATE));
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
        if cargo_toml.exists() && is_workspace_root(&cargo_toml) {
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
fn is_workspace_root(cargo_toml: &Utf8Path) -> bool {
    std::fs::read_to_string(cargo_toml)
        .ok()
        .and_then(|contents| contents.parse::<toml::Table>().ok())
        .is_some_and(|table| table.contains_key("workspace"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_crates_returns_all_by_default() {
        let crates = resolve_crates(&[], false, false);
        assert!(crates.contains(&CrateName::from("module_max_lines")));
        assert!(crates.contains(&CrateName::from(SUITE_CRATE)));
    }

    #[test]
    fn resolve_crates_suite_only() {
        let crates = resolve_crates(&[], true, false);
        assert_eq!(crates, vec![CrateName::from(SUITE_CRATE)]);
    }

    #[test]
    fn resolve_crates_no_suite() {
        let crates = resolve_crates(&[], false, true);
        assert!(!crates.contains(&CrateName::from(SUITE_CRATE)));
        assert!(crates.contains(&CrateName::from("module_max_lines")));
    }

    #[test]
    fn resolve_crates_specific_lints() {
        let specific = vec![CrateName::from("module_max_lines")];
        let crates = resolve_crates(&specific, false, false);
        assert_eq!(crates, vec![CrateName::from("module_max_lines")]);
    }

    #[test]
    fn validate_known_crates_succeeds() {
        let names = vec![
            CrateName::from("module_max_lines"),
            CrateName::from("suite"),
        ];
        assert!(validate_crate_names(&names).is_ok());
    }

    #[test]
    fn validate_unknown_crate_fails() {
        let names = vec![CrateName::from("nonexistent_lint")];
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
