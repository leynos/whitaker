//! Build and staging pipeline orchestration.
//!
//! This module provides higher-level orchestration for building lint crates and
//! staging the resulting libraries. It coordinates between the builder, stager,
//! and output modules to provide a complete build pipeline.

use crate::builder::{BuildConfig, BuildResult, Builder};
use crate::crate_name::CrateName;
use crate::error::Result;
use crate::output::write_stderr_line;
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

#[cfg(test)]
mod tests {
    //! Unit tests for pipeline orchestration.
    //!
    //! Full integration tests are impractical because `perform_build` and
    //! `stage_libraries` invoke cargo and perform filesystem operations. These
    //! tests focus on verifying progress output behaviour and configuration
    //! construction from `PipelineContext`.

    use super::*;
    use rstest::rstest;

    fn test_toolchain() -> Toolchain {
        Toolchain::with_override(&Utf8PathBuf::from("/tmp/test"), "nightly-2025-09-18")
    }

    fn test_context() -> (Utf8PathBuf, Utf8PathBuf, Toolchain) {
        let workspace_root = Utf8PathBuf::from("/tmp/workspace");
        let target_dir = Utf8PathBuf::from("/tmp/target");
        let toolchain = test_toolchain();
        (workspace_root, target_dir, toolchain)
    }

    #[rstest]
    #[case::quiet_mode(true)]
    #[case::verbose_mode(false)]
    fn perform_build_respects_quiet_flag(#[case] quiet: bool) {
        let (workspace_root, target_dir, toolchain) = test_context();
        let context = PipelineContext {
            workspace_root: &workspace_root,
            toolchain: &toolchain,
            target_dir: &target_dir,
            jobs: None,
            verbosity: 0,
            experimental: false,
            quiet,
        };
        let crates = vec![CrateName::from("suite")];
        let mut stderr = Vec::new();

        // perform_build will fail (no actual cargo build), but we're testing output
        let _ = perform_build(&context, &crates, &mut stderr);

        let output = String::from_utf8_lossy(&stderr);
        if quiet {
            assert!(output.is_empty(), "expected no output in quiet mode");
        } else {
            assert!(output.contains("Building"), "expected progress output");
            assert!(output.contains("suite"), "expected crate name in output");
        }
    }

    #[test]
    fn pipeline_context_fields_are_accessible() {
        let workspace_root = Utf8PathBuf::from("/workspace");
        let target_dir = Utf8PathBuf::from("/target");
        let toolchain = test_toolchain();

        let context = PipelineContext {
            workspace_root: &workspace_root,
            toolchain: &toolchain,
            target_dir: &target_dir,
            jobs: Some(4),
            verbosity: 2,
            experimental: true,
            quiet: false,
        };

        assert_eq!(context.workspace_root, Utf8Path::new("/workspace"));
        assert_eq!(context.target_dir, Utf8Path::new("/target"));
        assert_eq!(context.toolchain.channel(), "nightly-2025-09-18");
        assert_eq!(context.jobs, Some(4));
        assert_eq!(context.verbosity, 2);
        assert!(context.experimental);
        assert!(!context.quiet);
    }
}
