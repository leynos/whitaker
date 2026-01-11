//! Unit tests for pipeline orchestration.
//!
//! These tests verify that `build_config_from_context` correctly maps
//! `PipelineContext` fields to `BuildConfig`, that `perform_build_with`
//! correctly invokes the builder with the provided crates, and that
//! `stage_libraries` correctly stages build results.

use super::{PipelineContext, build_config_from_context, perform_build_with, stage_libraries};
use crate::builder::{BuildResult, MockCrateBuilder};
use crate::crate_name::CrateName;
use crate::toolchain::Toolchain;
use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use tempfile::TempDir;

// -------------------------------------------------------------------------
// Common trait for test context providers
// -------------------------------------------------------------------------

/// Common interface for test context fixtures that provide pipeline contexts.
trait PipelineContextProvider {
    /// Returns a pipeline context with borrowed references to internal state.
    fn pipeline_context(&self) -> PipelineContext<'_>;
}

// -------------------------------------------------------------------------
// TestContext for build_config and perform_build tests
// -------------------------------------------------------------------------

/// Fixture providing a default test context with paths owned by the returned struct.
struct TestContext {
    workspace_root: Utf8PathBuf,
    target_dir: Utf8PathBuf,
    toolchain: Toolchain,
    jobs: Option<usize>,
    verbosity: u8,
    experimental: bool,
    quiet: bool,
}

impl TestContext {
    fn new() -> Self {
        let base = Utf8PathBuf::from("test_workspace");
        Self {
            workspace_root: base.clone(),
            target_dir: base.join("target"),
            toolchain: Toolchain::with_override(&base, "nightly-2025-09-18"),
            jobs: None,
            verbosity: 0,
            experimental: false,
            quiet: false,
        }
    }

    fn with_jobs(mut self, jobs: Option<usize>) -> Self {
        self.jobs = jobs;
        self
    }

    fn with_verbosity(mut self, verbosity: u8) -> Self {
        self.verbosity = verbosity;
        self
    }

    fn with_experimental(mut self, experimental: bool) -> Self {
        self.experimental = experimental;
        self
    }

    fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }
}

impl PipelineContextProvider for TestContext {
    fn pipeline_context(&self) -> PipelineContext<'_> {
        PipelineContext {
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

/// Returns a default TestContext with owned paths and default settings for pipeline unit tests.
#[fixture]
fn test_ctx() -> TestContext {
    TestContext::new()
}

// -------------------------------------------------------------------------
// build_config_from_context tests
// -------------------------------------------------------------------------

#[rstest]
fn build_config_from_context_sets_toolchain(test_ctx: TestContext) {
    let config = build_config_from_context(&test_ctx.pipeline_context());
    assert_eq!(config.toolchain.channel(), "nightly-2025-09-18");
}

#[rstest]
fn build_config_from_context_sets_target_dir_to_workspace_target(test_ctx: TestContext) {
    let config = build_config_from_context(&test_ctx.pipeline_context());
    assert_eq!(
        config.target_dir,
        Utf8PathBuf::from("test_workspace/target")
    );
}

#[rstest]
#[case::no_jobs(None)]
#[case::four_jobs(Some(4))]
#[case::single_job(Some(1))]
fn build_config_from_context_sets_jobs(#[case] jobs: Option<usize>) {
    let ctx = TestContext::new().with_jobs(jobs);
    assert_eq!(
        build_config_from_context(&ctx.pipeline_context()).jobs,
        jobs
    );
}

#[rstest]
#[case::silent(0)]
#[case::verbose(1)]
#[case::very_verbose(3)]
fn build_config_from_context_sets_verbosity(#[case] verbosity: u8) {
    let ctx = TestContext::new().with_verbosity(verbosity);
    assert_eq!(
        build_config_from_context(&ctx.pipeline_context()).verbosity,
        verbosity
    );
}

#[rstest]
#[case::stable(false)]
#[case::experimental(true)]
fn build_config_from_context_sets_experimental(#[case] exp: bool) {
    let ctx = TestContext::new().with_experimental(exp);
    assert_eq!(
        build_config_from_context(&ctx.pipeline_context()).experimental,
        exp
    );
}

// -------------------------------------------------------------------------
// perform_build_with mockall tests
// -------------------------------------------------------------------------

#[rstest]
fn perform_build_with_calls_build_all_with_provided_crates(test_ctx: TestContext) {
    let ctx = test_ctx.with_quiet(true);
    let crates = vec![
        CrateName::from("whitaker_suite"),
        CrateName::from("module_max_lines"),
    ];

    let mut mock = MockCrateBuilder::new();
    mock.expect_build_all()
        .withf(|c| {
            c.len() == 2 && c[0].as_str() == "whitaker_suite" && c[1].as_str() == "module_max_lines"
        })
        .times(1)
        .returning(|_| Ok(vec![]));

    let mut stderr = Vec::new();
    assert!(perform_build_with(&ctx.pipeline_context(), &crates, &mock, &mut stderr).is_ok());
}

#[rstest]
fn perform_build_with_returns_builder_results(test_ctx: TestContext) {
    let ctx = test_ctx.with_quiet(true);
    let crates = vec![CrateName::from("whitaker_suite")];

    let mut mock = MockCrateBuilder::new();
    mock.expect_build_all().times(1).returning(|_| {
        Ok(vec![BuildResult {
            crate_name: CrateName::from("whitaker_suite"),
            library_path: Utf8PathBuf::from("/path/to/libwhitaker_suite.so"),
        }])
    });

    let mut stderr = Vec::new();
    let results = perform_build_with(&ctx.pipeline_context(), &crates, &mock, &mut stderr)
        .expect("build should succeed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].crate_name.as_str(), "whitaker_suite");
}

#[rstest]
#[case::quiet_mode(true)]
#[case::verbose_mode(false)]
fn perform_build_with_respects_quiet_flag(test_ctx: TestContext, #[case] quiet: bool) {
    let ctx = test_ctx.with_quiet(quiet);
    let crates = vec![CrateName::from("whitaker_suite")];

    let mut mock = MockCrateBuilder::new();
    mock.expect_build_all().times(1).returning(|_| Ok(vec![]));

    let mut stderr = Vec::new();
    perform_build_with(&ctx.pipeline_context(), &crates, &mock, &mut stderr)
        .expect("build should succeed");

    let output = String::from_utf8_lossy(&stderr);
    if quiet {
        assert!(output.is_empty(), "expected no output in quiet mode");
    } else {
        assert!(output.contains("Building"), "expected progress output");
        assert!(
            output.contains("whitaker_suite"),
            "expected crate name in output"
        );
    }
}

#[test]
fn pipeline_context_fields_are_accessible() {
    let ctx = TestContext::new()
        .with_jobs(Some(4))
        .with_verbosity(2)
        .with_experimental(true);
    let context = ctx.pipeline_context();

    assert_eq!(context.workspace_root, Utf8Path::new("test_workspace"));
    assert_eq!(context.target_dir, Utf8Path::new("test_workspace/target"));
    assert_eq!(context.toolchain.channel(), "nightly-2025-09-18");
    assert_eq!(context.jobs, Some(4));
    assert_eq!(context.verbosity, 2);
    assert!(context.experimental);
    assert!(!context.quiet);
}

// -------------------------------------------------------------------------
// stage_libraries tests
// -------------------------------------------------------------------------

/// Fixture providing a temporary directory for staging tests.
///
/// Contains its own fields for real file system operations during staging tests,
/// mirroring `TestContext` but with a real temporary directory for `target_dir`.
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
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let target_dir =
            Utf8PathBuf::try_from(temp_dir.path().to_owned()).expect("non-UTF8 temp path");
        Self {
            _temp_dir: temp_dir,
            target_dir,
            workspace_root: Utf8PathBuf::from("/tmp/workspace"),
            toolchain: Toolchain::with_override(
                &Utf8PathBuf::from("/tmp/test"),
                "nightly-2025-09-18",
            ),
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

    #[expect(dead_code, reason = "API parity with TestContext for future test use")]
    fn with_jobs(mut self, jobs: Option<usize>) -> Self {
        self.jobs = jobs;
        self
    }

    #[expect(dead_code, reason = "API parity with TestContext for future test use")]
    fn with_verbosity(mut self, verbosity: u8) -> Self {
        self.verbosity = verbosity;
        self
    }

    fn with_experimental(mut self, experimental: bool) -> Self {
        self.experimental = experimental;
        self
    }

    fn pipeline_context(&self) -> PipelineContext<'_> {
        PipelineContext {
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

fn assert_experimental_lint_in_staging_output(experimental: bool, expect_experimental: bool) {
    let staging_ctx = StagingTestContext::new().with_experimental(experimental);
    let context = staging_ctx.pipeline_context();
    let build_results = vec![create_mock_library(
        staging_ctx.target_dir(),
        "whitaker_suite",
    )];
    let mut stderr = Vec::new();

    stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

    let output = String::from_utf8_lossy(&stderr);
    if expect_experimental {
        assert!(
            output.contains("bumpy_road_function"),
            "expected experimental lint in output, got: {output}"
        );
    } else {
        assert!(
            !output.contains("bumpy_road_function"),
            "did not expect experimental lint in output, got: {output}"
        );
    }
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

    // Verify the path matches the expected Stager::staging_path() format
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

    // Verify the library was staged to the correct location
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
fn stage_libraries_includes_experimental_lints_when_enabled() {
    assert_experimental_lint_in_staging_output(true, true);
}

#[rstest]
fn stage_libraries_excludes_experimental_lints_when_disabled() {
    assert_experimental_lint_in_staging_output(false, false);
}
