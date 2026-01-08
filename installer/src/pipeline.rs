//! Build and staging pipeline orchestration.
//!
//! This module provides higher-level orchestration for building lint crates and
//! staging the resulting libraries. It coordinates between the builder, stager,
//! and output modules to provide a complete build pipeline.

use crate::builder::{BuildConfig, BuildResult, Builder};
use crate::crate_name::CrateName;
use crate::error::Result;
use crate::scanner::lints_for_library;
use crate::stager::Stager;
use crate::toolchain::Toolchain;
use camino::{Utf8Path, Utf8PathBuf};
use std::io::Write;

/// Context for a build/stage pipeline run.
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

    // Build artifacts go to {workspace_root}/target (Cargo's standard location),
    // distinct from context.target_dir which is the user-facing staging directory
    // (e.g., ~/.local/share/dylint/lib) where final libraries are copied.
    let config = BuildConfig {
        toolchain: context.toolchain.clone(),
        target_dir: context.workspace_root.join("target"),
        jobs: context.jobs,
        verbosity: context.verbosity,
        experimental: context.experimental,
    };
    Builder::new(config).build_all(crates)
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
    use crate::output::success_message;

    let stager = Stager::new(context.target_dir.to_owned(), context.toolchain.channel());
    let staging_path = stager.staging_path();

    if !context.quiet {
        write_stderr_line(stderr, format!("Staging libraries to {staging_path}..."));
    }

    stager.prepare()?;
    stager.stage_all(build_results)?;

    if !context.quiet {
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, success_message(build_results.len(), &staging_path));
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "Installed lints:");
        for result in build_results {
            let lint_names = lints_for_library(&result.crate_name);
            for lint in lint_names {
                write_stderr_line(stderr, format!("  - {lint}"));
            }
        }
    }

    Ok(staging_path)
}

fn write_stderr_line(stderr: &mut dyn Write, message: impl std::fmt::Display) {
    if writeln!(stderr, "{message}").is_err() {
        // Best-effort logging; ignore write failures.
    }
}
