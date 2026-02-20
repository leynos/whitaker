//! Staging-focused tests for pipeline orchestration.

use crate::builder::BuildResult;
use crate::crate_name::CrateName;
use crate::pipeline::stage_libraries;
use crate::toolchain::Toolchain;
use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use tempfile::TempDir;

/// Fixture providing a temporary directory for staging tests.
///
/// Contains its own fields for real file system operations during staging
/// tests, mirroring the parent test context but with a real temporary
/// directory for `target_dir`.
struct StagingTestContext {
    _temp_dir: TempDir,
    target_dir: Utf8PathBuf,
    workspace_root: Utf8PathBuf,
    toolchain: Toolchain,
    jobs: Option<usize>,
    verbosity: u8,
    experimental: bool,
    quiet: bool,
}

impl StagingTestContext {
    fn new() -> Self {
        use std::fs;

        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let target_dir =
            Utf8PathBuf::try_from(temp_dir.path().to_owned()).expect("non-UTF8 temp path");
        let workspace_root = target_dir.join("workspace");
        fs::create_dir_all(&workspace_root).expect("failed to create workspace root");
        Self {
            _temp_dir: temp_dir,
            target_dir,
            toolchain: Toolchain::with_override(&workspace_root, "nightly-2025-09-18"),
            workspace_root,
            jobs: None,
            verbosity: 0,
            experimental: false,
            quiet: false,
        }
    }

    fn target_dir(&self) -> &Utf8Path {
        &self.target_dir
    }

    fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    fn with_experimental(mut self, experimental: bool) -> Self {
        self.experimental = experimental;
        self
    }

    fn pipeline_context(&self) -> super::PipelineContext<'_> {
        super::PipelineContext {
            workspace_root: &self.workspace_root,
            toolchain: &self.toolchain,
            target_dir: &self.target_dir,
            jobs: self.jobs,
            verbosity: self.verbosity,
            experimental: self.experimental,
            quiet: self.quiet,
        }
    }
}

fn create_mock_library(target_dir: &Utf8Path, crate_name: &str) -> BuildResult {
    use crate::builder::{library_extension, library_prefix};
    use std::fs;

    let source_dir = target_dir.join("source");
    fs::create_dir_all(&source_dir).expect("failed to create source directory");
    let filename = format!("{}{}{}", library_prefix(), crate_name, library_extension());
    let library_path = source_dir.join(&filename);
    fs::write(&library_path, b"mock library content").expect("failed to write mock library");

    BuildResult {
        crate_name: CrateName::from(crate_name),
        library_path,
    }
}

#[fixture]
fn staging_ctx() -> StagingTestContext {
    StagingTestContext::new()
}

fn assert_bumpy_road_lint_in_staging_output(experimental: bool) {
    let staging_ctx = StagingTestContext::new().with_experimental(experimental);
    let context = staging_ctx.pipeline_context();
    let build_results = vec![create_mock_library(
        staging_ctx.target_dir(),
        "whitaker_suite",
    )];
    let mut stderr = Vec::new();

    stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

    let output = String::from_utf8_lossy(&stderr);
    assert!(
        output.contains("bumpy_road_function"),
        "expected stable bumpy_road_function lint in output, got: {output}"
    );
}

#[rstest]
fn stage_libraries_returns_correct_staging_path(staging_ctx: StagingTestContext) {
    let staging_ctx = staging_ctx.with_quiet(true);
    let context = staging_ctx.pipeline_context();
    let build_results = vec![];
    let mut stderr = Vec::new();

    let result = stage_libraries(&context, &build_results, &mut stderr);

    assert!(result.is_ok(), "expected success, got: {result:?}");
    let staging_path = result.expect("already checked");

    // Keep this contract explicit so staged artefacts remain discoverable by
    // toolchain and profile when scanner logic depends on path layout.
    let expected_path = staging_ctx
        .target_dir()
        .join("nightly-2025-09-18")
        .join("release");
    assert_eq!(
        staging_path, expected_path,
        "staging path should match Stager format"
    );
}

#[rstest]
#[case::quiet_mode(true)]
#[case::verbose_mode(false)]
fn stage_libraries_respects_quiet_flag(staging_ctx: StagingTestContext, #[case] quiet: bool) {
    let staging_ctx = staging_ctx.with_quiet(quiet);
    let context = staging_ctx.pipeline_context();
    let build_results = vec![];
    let mut stderr = Vec::new();

    stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

    let output = String::from_utf8_lossy(&stderr);
    if quiet {
        assert!(output.is_empty(), "expected no output in quiet mode");
    } else {
        assert!(
            output.contains("Staging libraries to"),
            "expected progress message, got: {}",
            output
        );
    }
}

#[rstest]
fn stage_libraries_stages_build_results(staging_ctx: StagingTestContext) {
    use crate::builder::{library_extension, library_prefix};

    let staging_ctx = staging_ctx.with_quiet(true);
    let context = staging_ctx.pipeline_context();
    let build_results = vec![create_mock_library(
        staging_ctx.target_dir(),
        "whitaker_suite",
    )];
    let mut stderr = Vec::new();

    let staging_path =
        stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

    // The staged filename must preserve crate and toolchain identity so
    // multi-toolchain installs do not collide.
    let staged_filename = format!(
        "{}whitaker_suite@nightly-2025-09-18{}",
        library_prefix(),
        library_extension()
    );
    let staged_library = staging_path.join(&staged_filename);
    assert!(
        staged_library.exists(),
        "expected staged library at {staged_library}"
    );
}

#[rstest]
fn stage_libraries_logs_installed_lints_when_not_quiet(staging_ctx: StagingTestContext) {
    let context = staging_ctx.pipeline_context();
    let build_results = vec![];
    let mut stderr = Vec::new();

    stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

    let output = String::from_utf8_lossy(&stderr);
    assert!(
        output.contains("Installed lints:"),
        "expected installed lints section in verbose output"
    );
}

#[rstest]
#[case::without_experimental(false)]
#[case::with_experimental(true)]
fn stage_libraries_lists_bumpy_road_lint(#[case] experimental: bool) {
    assert_bumpy_road_lint_in_staging_output(experimental);
}
