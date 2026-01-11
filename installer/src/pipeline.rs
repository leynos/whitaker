//! Build and staging pipeline orchestration.
//!
//! This module provides higher-level orchestration for building lint crates and
//! staging the resulting libraries. It coordinates between the builder, stager,
//! and output modules to provide a complete build pipeline.

use crate::builder::{BuildConfig, BuildResult, Builder, CrateBuilder};
use crate::crate_name::CrateName;
use crate::error::Result;
use crate::output::{success_message, write_stderr_line};
use crate::scanner::lints_for_library_with_experimental;
use crate::stager::Stager;
use crate::toolchain::Toolchain;
use camino::{Utf8Path, Utf8PathBuf};
use std::io::Write;

/// Creates a [`BuildConfig`] from the pipeline context.
///
/// This extracts the build configuration parameters from the pipeline context
/// for use with a [`Builder`]. The `target_dir` in the resulting config points
/// to the cargo target directory (`{workspace_root}/target`), not the staging
/// directory.
///
/// # Example
///
/// ```
/// use whitaker_installer::pipeline::{build_config_from_context, PipelineContext};
/// use whitaker_installer::toolchain::Toolchain;
/// use camino::{Utf8Path, Utf8PathBuf};
///
/// let workspace = Utf8PathBuf::from("/workspace");
/// let target = Utf8PathBuf::from("/staging");
/// let toolchain = Toolchain::with_override(&workspace, "nightly-2025-01-01");
///
/// let ctx = PipelineContext {
///     workspace_root: &workspace,
///     toolchain: &toolchain,
///     target_dir: &target,
///     jobs: Some(4),
///     verbosity: 1,
///     experimental: false,
///     quiet: false,
/// };
///
/// let config = build_config_from_context(&ctx);
/// assert_eq!(config.target_dir, Utf8PathBuf::from("/workspace/target"));
/// assert_eq!(config.jobs, Some(4));
/// ```
#[must_use]
pub fn build_config_from_context(context: &PipelineContext<'_>) -> BuildConfig {
    BuildConfig {
        toolchain: context.toolchain.clone(),
        target_dir: context.workspace_root.join("target"),
        jobs: context.jobs,
        verbosity: context.verbosity,
        experimental: context.experimental,
    }
}

/// Context for a build/stage pipeline run.
///
/// `PipelineContext` aggregates the configuration needed to build lint crates
/// and stage the resulting libraries. It is passed to [`perform_build`] and
/// [`stage_libraries`] to coordinate the build pipeline.
///
/// All fields are borrowed references to avoid ownership transfer, allowing
/// the context to be reused across multiple pipeline operations.
///
/// # Example
///
/// ```
/// use whitaker_installer::pipeline::PipelineContext;
/// use whitaker_installer::toolchain::Toolchain;
/// use camino::Utf8PathBuf;
///
/// let workspace = Utf8PathBuf::from("/workspace");
/// let target = Utf8PathBuf::from("/staging");
/// let toolchain = Toolchain::with_override(&workspace, "nightly-2025-01-01");
///
/// let ctx = PipelineContext {
///     workspace_root: &workspace,
///     toolchain: &toolchain,
///     target_dir: &target,
///     jobs: Some(4),
///     verbosity: 1,
///     experimental: false,
///     quiet: false,
/// };
///
/// assert_eq!(ctx.jobs, Some(4));
/// assert!(!ctx.quiet);
/// ```
pub struct PipelineContext<'a> {
    /// Workspace root directory.
    pub workspace_root: &'a Utf8Path,
    /// Detected or overridden toolchain.
    pub toolchain: &'a Toolchain,
    /// Target directory for staging.
    pub target_dir: &'a Utf8Path,
    /// Number of parallel build jobs.
    pub jobs: Option<usize>,
    /// Verbosity level.
    pub verbosity: u8,
    /// Whether to include experimental features.
    pub experimental: bool,
    /// Suppress progress output.
    pub quiet: bool,
}

/// Builds all requested crates.
///
/// Prints progress to stderr if not in quiet mode.
///
/// # Errors
///
/// Returns an error if any crate fails to build.
pub fn perform_build(
    context: &PipelineContext<'_>,
    crates: &[CrateName],
    stderr: &mut dyn Write,
) -> Result<Vec<BuildResult>> {
    let config = build_config_from_context(context);
    let builder = Builder::new(config);
    perform_build_with(context, crates, &builder, stderr)
}

/// Builds all requested crates using the provided builder.
///
/// This is the internal implementation that accepts a [`CrateBuilder`] trait
/// object for dependency injection, enabling tests to mock the build process.
///
/// # Errors
///
/// Returns an error if any crate fails to build.
pub(crate) fn perform_build_with(
    context: &PipelineContext<'_>,
    crates: &[CrateName],
    builder: &dyn CrateBuilder,
    stderr: &mut dyn Write,
) -> Result<Vec<BuildResult>> {
    if !context.quiet {
        write_stderr_line(
            stderr,
            format!(
                "Building {} lint crate(s) with toolchain {}...",
                crates.len(),
                context.toolchain.channel()
            ),
        );
        write_stderr_line(stderr, "  Crates to build:");
        for crate_name in crates {
            write_stderr_line(stderr, format!("    - {crate_name}"));
        }
        write_stderr_line(stderr, "");
    }

    builder.build_all(crates)
}

/// Stages built libraries and returns the staging path.
///
/// Prints progress to stderr if not in quiet mode.
///
/// # Errors
///
/// Returns an error if staging fails.
pub fn stage_libraries(
    context: &PipelineContext<'_>,
    build_results: &[BuildResult],
    stderr: &mut dyn Write,
) -> Result<Utf8PathBuf> {
    let stager = Stager::new(context.target_dir.to_owned(), context.toolchain.channel());
    let staging_path = stager.staging_path();

    if !context.quiet {
        write_stderr_line(stderr, format!("Staging libraries to {staging_path}..."));
    }

    stager.prepare()?;
    stager.stage_all(build_results)?;

    if !context.quiet {
        log_staging_results(stderr, build_results, &staging_path, context.experimental);
    }

    Ok(staging_path)
}

/// Logs the staging results to stderr.
fn log_staging_results(
    stderr: &mut dyn Write,
    build_results: &[BuildResult],
    staging_path: &Utf8Path,
    include_experimental: bool,
) {
    write_stderr_line(stderr, "");
    write_stderr_line(stderr, success_message(build_results.len(), staging_path));
    write_stderr_line(stderr, "");
    write_stderr_line(stderr, "Installed lints:");
    for result in build_results {
        for lint in lints_for_library_with_experimental(&result.crate_name, include_experimental) {
            write_stderr_line(stderr, format!("  - {lint}"));
        }
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
