//! Cargo build orchestration for lint crates.
//!
//! This module provides utilities to build Dylint lint crates in release mode
//! with the required features enabled.

use std::process::Command;

use camino::Utf8PathBuf;

mod revision;

// Re-export from submodules for backwards compatibility
#[cfg(test)]
pub(crate) use revision::MockSuiteRevisionProbe;
use revision::{GitSuiteRevisionProbe, SuiteRevisionProbe, apply_suite_revision, resolve_revision};

pub use crate::{
    crate_name::CrateName,
    resolution::{
        CrateResolutionOptions,
        EXPERIMENTAL_LINT_CRATES,
        LINT_CRATES,
        SUITE_CRATE,
        is_known_crate,
        resolve_crates,
        validate_crate_names,
    },
    workspace::find_workspace_root,
};
use crate::{
    error::{InstallerError, Result},
    suite_features::experimental_features,
    toolchain::Toolchain,
};

/// Configuration for the build process.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// The Rust toolchain to use.
    pub toolchain: Toolchain,
    /// Directory for build artefacts.
    pub target_dir: Utf8PathBuf,
    /// Number of parallel build jobs (None for cargo default).
    pub jobs: Option<usize>,
    /// Verbosity level for cargo output.
    pub verbosity: u8,
    /// Build the suite with every experimental lint enabled by default.
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
    /// Resolves the checkout's suite revision; injectable so a test can
    /// substitute a probe that fails or returns a known value.
    probe: Box<dyn SuiteRevisionProbe>,
}

impl Builder {
    /// Create a new builder with the given configuration.
    #[must_use]
    pub fn new(config: BuildConfig) -> Self {
        Self {
            config,
            probe: Box::new(GitSuiteRevisionProbe),
        }
    }

    /// Create a builder with an explicit revision probe.
    #[cfg(test)]
    pub(crate) fn with_probe(config: BuildConfig, probe: Box<dyn SuiteRevisionProbe>) -> Self {
        Self { config, probe }
    }

    /// Build a single lint crate.
    ///
    /// # Errors
    ///
    /// Returns an error if the cargo build command fails.
    pub fn build_crate(&self, crate_name: &CrateName) -> Result<BuildResult> {
        let mut cmd = self.build_command(crate_name)?;
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

    /// Assemble the `cargo build` command for one crate.
    ///
    /// Kept separate from [`Self::build_crate`] so the command a build would
    /// run — including the revision it publishes — can be inspected without
    /// executing cargo.
    ///
    /// # Errors
    ///
    /// Returns an error when the crate's feature set cannot be resolved.
    fn build_command(&self, crate_name: &CrateName) -> Result<Command> {
        let mut cmd = Command::new("cargo");

        cmd.arg(format!("+{}", self.config.toolchain.channel()));
        cmd.args(["build", "--release"]);

        let features = self.features_for_crate(crate_name)?;
        cmd.args(["--features", &features]);

        cmd.args(["-p", crate_name.as_str()]);

        if let Some(jobs) = self.config.jobs {
            cmd.args(["-j", &jobs.to_string()]);
        }

        cmd.env("CARGO_TARGET_DIR", self.config.target_dir.as_str());
        cmd.current_dir(self.config.toolchain.workspace_root());

        // Source builds identify the suite they compiled. A build from a
        // checkout with uncommitted changes is marked so a warning never
        // claims provenance the binary does not have.
        let revision =
            resolve_revision(self.probe.as_ref(), self.config.toolchain.workspace_root());
        apply_suite_revision(&mut cmd, revision.as_deref());

        for _ in 0..self.config.verbosity {
            cmd.arg("-v");
        }

        Ok(cmd)
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
    /// Every suite compiles in all experimental lints, so a plain suite build
    /// needs no per-lint feature. With `--experimental` the suite also gets the
    /// feature that starts every experimental lint enabled (a repository's
    /// `dylint.toml` still adds to that): `experimental-enable-all` for a
    /// suite from 0.3.0, or the per-lint features an older pinned suite
    /// declares, read from its own manifest. Individual lint crates need only
    /// the `dylint-driver` feature.
    ///
    /// # Errors
    ///
    /// Returns an error when the suite's manifest cannot be read or parsed.
    fn features_for_crate(&self, crate_name: &CrateName) -> Result<String> {
        if crate_name.as_str() != SUITE_CRATE || !self.config.experimental {
            return Ok("dylint-driver".to_owned());
        }
        let experimental = experimental_features(self.config.toolchain.workspace_root())?;
        Ok(if experimental.is_empty() {
            "dylint-driver".to_owned()
        } else {
            format!("dylint-driver,{experimental}")
        })
    }

    /// Returns a reference to the build configuration.
    ///
    /// This method is primarily useful for testing to verify that the correct
    /// configuration was constructed.
    #[must_use]
    pub const fn config(&self) -> &BuildConfig { &self.config }
}

impl CrateBuilder for Builder {
    fn build_all(&self, crates: &[CrateName]) -> Result<Vec<BuildResult>> { self.build_all(crates) }
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
#[path = "builder_tests.rs"]
mod tests;
