//! Unit tests for pipeline orchestration.
//!
//! These tests verify that `build_config_from_context` correctly maps
//! `PipelineContext` fields to `BuildConfig`, and that `perform_build_with`
//! correctly invokes the builder with the provided crates.

use super::*;
use crate::builder::MockCrateBuilder;
use rstest::{fixture, rstest};

/// Fixture providing a test toolchain.
#[fixture]
fn toolchain() -> Toolchain {
    Toolchain::with_override(&Utf8PathBuf::from("/tmp/test"), "nightly-2025-09-18")
}

/// Fixture providing a default test context with paths owned by the returned struct.
struct TestContext {
    workspace_root: Utf8PathBuf,
    target_dir: Utf8PathBuf,
    toolchain: Toolchain,
}

impl TestContext {
    fn new() -> Self {
        Self {
            workspace_root: Utf8PathBuf::from("/tmp/workspace"),
            target_dir: Utf8PathBuf::from("/tmp/target"),
            toolchain: Toolchain::with_override(
                &Utf8PathBuf::from("/tmp/test"),
                "nightly-2025-09-18",
            ),
        }
    }

    fn pipeline_context(&self, quiet: bool) -> PipelineContext<'_> {
        PipelineContext {
            workspace_root: &self.workspace_root,
            toolchain: &self.toolchain,
            target_dir: &self.target_dir,
            jobs: None,
            verbosity: 0,
            experimental: false,
            quiet,
        }
    }
}

#[fixture]
fn test_ctx() -> TestContext {
    TestContext::new()
}

// -------------------------------------------------------------------------
// build_config_from_context tests
// -------------------------------------------------------------------------

#[rstest]
fn build_config_from_context_sets_toolchain(test_ctx: TestContext) {
    let config = build_config_from_context(&test_ctx.pipeline_context(false));
    assert_eq!(config.toolchain.channel(), "nightly-2025-09-18");
}

#[rstest]
fn build_config_from_context_sets_target_dir_to_workspace_target(test_ctx: TestContext) {
    let config = build_config_from_context(&test_ctx.pipeline_context(false));
    assert_eq!(
        config.target_dir,
        Utf8PathBuf::from("/tmp/workspace/target")
    );
}

#[rstest]
#[case::no_jobs(None)]
#[case::four_jobs(Some(4))]
#[case::single_job(Some(1))]
fn build_config_from_context_sets_jobs(toolchain: Toolchain, #[case] jobs: Option<usize>) {
    let workspace_root = Utf8PathBuf::from("/workspace");
    let target_dir = Utf8PathBuf::from("/staging");
    let context = PipelineContext {
        workspace_root: &workspace_root,
        toolchain: &toolchain,
        target_dir: &target_dir,
        jobs,
        verbosity: 0,
        experimental: false,
        quiet: false,
    };
    assert_eq!(build_config_from_context(&context).jobs, jobs);
}

#[rstest]
#[case::silent(0)]
#[case::verbose(1)]
#[case::very_verbose(3)]
fn build_config_from_context_sets_verbosity(toolchain: Toolchain, #[case] verbosity: u8) {
    let workspace_root = Utf8PathBuf::from("/workspace");
    let target_dir = Utf8PathBuf::from("/staging");
    let context = PipelineContext {
        workspace_root: &workspace_root,
        toolchain: &toolchain,
        target_dir: &target_dir,
        jobs: None,
        verbosity,
        experimental: false,
        quiet: false,
    };
    assert_eq!(build_config_from_context(&context).verbosity, verbosity);
}

#[rstest]
#[case::stable(false)]
#[case::experimental(true)]
fn build_config_from_context_sets_experimental(toolchain: Toolchain, #[case] exp: bool) {
    let workspace_root = Utf8PathBuf::from("/workspace");
    let target_dir = Utf8PathBuf::from("/staging");
    let context = PipelineContext {
        workspace_root: &workspace_root,
        toolchain: &toolchain,
        target_dir: &target_dir,
        jobs: None,
        verbosity: 0,
        experimental: exp,
        quiet: false,
    };
    assert_eq!(build_config_from_context(&context).experimental, exp);
}

// -------------------------------------------------------------------------
// perform_build_with mockall tests
// -------------------------------------------------------------------------

#[rstest]
fn perform_build_with_calls_build_all_with_provided_crates(test_ctx: TestContext) {
    let context = test_ctx.pipeline_context(true);
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
    assert!(perform_build_with(&context, &crates, &mock, &mut stderr).is_ok());
}

#[rstest]
fn perform_build_with_returns_builder_results(test_ctx: TestContext) {
    let context = test_ctx.pipeline_context(true);
    let crates = vec![CrateName::from("suite")];

    let mut mock = MockCrateBuilder::new();
    mock.expect_build_all().times(1).returning(|_| {
        Ok(vec![BuildResult {
            crate_name: CrateName::from("suite"),
            library_path: Utf8PathBuf::from("/path/to/libsuite.so"),
        }])
    });

    let mut stderr = Vec::new();
    let results =
        perform_build_with(&context, &crates, &mock, &mut stderr).expect("build should succeed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].crate_name.as_str(), "suite");
}

#[rstest]
#[case::quiet_mode(true)]
#[case::verbose_mode(false)]
fn perform_build_with_respects_quiet_flag(test_ctx: TestContext, #[case] quiet: bool) {
    let context = test_ctx.pipeline_context(quiet);
    let crates = vec![CrateName::from("suite")];

    let mut mock = MockCrateBuilder::new();
    mock.expect_build_all().times(1).returning(|_| Ok(vec![]));

    let mut stderr = Vec::new();
    let _ = perform_build_with(&context, &crates, &mock, &mut stderr);

    let output = String::from_utf8_lossy(&stderr);
    if quiet {
        assert!(output.is_empty(), "expected no output in quiet mode");
    } else {
        assert!(output.contains("Building"), "expected progress output");
        assert!(output.contains("suite"), "expected crate name in output");
    }
}

#[rstest]
fn pipeline_context_fields_are_accessible(toolchain: Toolchain) {
    let workspace_root = Utf8PathBuf::from("/workspace");
    let target_dir = Utf8PathBuf::from("/target");
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
