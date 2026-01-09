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
        CrateName::from("suite"),
        CrateName::from("module_max_lines"),
    ];

    let mut mock = MockCrateBuilder::new();
    mock.expect_build_all()
        .withf(|c| c.len() == 2 && c[0].as_str() == "suite" && c[1].as_str() == "module_max_lines")
        .times(1)
        .returning(|_| Ok(vec![]));

    let mut stderr = Vec::new();
    assert!(perform_build_with(&ctx.pipeline_context(), &crates, &mock, &mut stderr).is_ok());
}

#[rstest]
fn perform_build_with_returns_builder_results(test_ctx: TestContext) {
    let ctx = test_ctx.with_quiet(true);
    let crates = vec![CrateName::from("suite")];

    let mut mock = MockCrateBuilder::new();
    mock.expect_build_all().times(1).returning(|_| {
        Ok(vec![BuildResult {
            crate_name: CrateName::from("suite"),
            library_path: Utf8PathBuf::from("/path/to/libsuite.so"),
        }])
    });

    let mut stderr = Vec::new();
    let results = perform_build_with(&ctx.pipeline_context(), &crates, &mock, &mut stderr)
        .expect("build should succeed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].crate_name.as_str(), "suite");
}

#[rstest]
#[case::quiet_mode(true)]
#[case::verbose_mode(false)]
fn perform_build_with_respects_quiet_flag(test_ctx: TestContext, #[case] quiet: bool) {
    let ctx = test_ctx.with_quiet(quiet);
    let crates = vec![CrateName::from("suite")];

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
        assert!(output.contains("suite"), "expected crate name in output");
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
/// Wraps a `TestContext` and adds a temporary directory for real file system
/// operations during staging tests.
struct StagingTestContext {
    _temp_dir: TempDir,
    ctx: TestContext,
}

impl StagingTestContext {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let target_dir =
            Utf8PathBuf::try_from(temp_dir.path().to_owned()).expect("non-UTF8 temp path");
        Self {
            _temp_dir: temp_dir,
            ctx: TestContext {
                target_dir,
                ..TestContext::new()
            },
        }
    }

    fn target_dir(&self) -> &Utf8Path {
        &self.ctx.target_dir
    }

    fn with_quiet(mut self, quiet: bool) -> Self {
        self.ctx.quiet = quiet;
        self
    }

    fn pipeline_context(&self) -> PipelineContext<'_> {
        self.ctx.pipeline_context()
    }
}

#[fixture]
fn staging_ctx() -> StagingTestContext {
    StagingTestContext::new()
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
