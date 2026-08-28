//! Staging-focused tests for pipeline orchestration.

use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use tempfile::TempDir;

use crate::{
    builder::BuildResult, crate_name::CrateName, pipeline::stage_libraries, toolchain::Toolchain,
};

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
    fn new() -> std::io::Result<Self> {
        use std::fs;

        let temp_dir = TempDir::new()?;
        let target_dir = Utf8PathBuf::try_from(temp_dir.path().to_owned())
            .map_err(|_| std::io::Error::other("temporary directory path must be UTF-8"))?;
        let workspace_root = target_dir.join("workspace");
        fs::create_dir_all(&workspace_root)?;
        Ok(Self {
            _temp_dir: temp_dir,
            target_dir,
            toolchain: Toolchain::with_override(&workspace_root, "nightly-2026-05-28"),
            workspace_root,
            jobs: None,
            verbosity: 0,
            experimental: false,
            quiet: false,
        })
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

fn create_mock_library(target_dir: &Utf8Path, crate_name: &str) -> std::io::Result<BuildResult> {
    use std::fs;

    use crate::builder::{library_extension, library_prefix};

    let source_dir = target_dir.join("source");
    fs::create_dir_all(&source_dir)?;
    let filename = format!("{}{}{}", library_prefix(), crate_name, library_extension());
    let library_path = source_dir.join(&filename);
    fs::write(&library_path, b"mock library content")?;

    Ok(BuildResult {
        crate_name: CrateName::from(crate_name),
        library_path,
    })
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn staging_ctx() -> std::io::Result<StagingTestContext> {
    StagingTestContext::new()
}

/// Asserts that staging output lists the stable `bumpy_road_function` lint.
///
/// Expressed as a macro so the fallible setup stays inside the calling test
/// body and failures report the caller's line number.
macro_rules! assert_bumpy_road_lint_in_staging_output {
    ($experimental:expr) => {{
        let staging_ctx = StagingTestContext::new()
            .expect("staging context should be created")
            .with_experimental($experimental);
        let context = staging_ctx.pipeline_context();
        let build_results = vec![
            create_mock_library(staging_ctx.target_dir(), "whitaker_suite")
                .expect("mock library should be staged"),
        ];
        let mut stderr = Vec::new();

        stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

        let output = String::from_utf8_lossy(&stderr);
        assert!(
            output.contains("bumpy_road_function"),
            "expected stable bumpy_road_function lint in output, got: {output}"
        );
    }};
}

#[rstest]
fn stage_libraries_returns_correct_staging_path(
    #[from(staging_ctx)] staging_ctx_res: std::io::Result<StagingTestContext>,
) {
    let staging_ctx = staging_ctx_res.expect("staging context should be created");

    let quiet_ctx = staging_ctx.with_quiet(true);
    let context = quiet_ctx.pipeline_context();
    let build_results = vec![];
    let mut stderr = Vec::new();

    let staging_path =
        stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

    // Keep this contract explicit so staged artefacts remain discoverable by
    // toolchain and profile when scanner logic depends on path layout.
    let expected_path = quiet_ctx
        .target_dir()
        .join("nightly-2026-05-28")
        .join("release");
    assert_eq!(
        staging_path, expected_path,
        "staging path should match Stager format"
    );
}

#[rstest]
#[case::quiet_mode(true)]
#[case::verbose_mode(false)]
fn stage_libraries_respects_quiet_flag(
    #[from(staging_ctx)] staging_ctx_res: std::io::Result<StagingTestContext>,
    #[case] quiet: bool,
) {
    let staging_ctx = staging_ctx_res.expect("staging context should be created");
    let quiet_ctx = staging_ctx.with_quiet(quiet);
    let context = quiet_ctx.pipeline_context();
    let build_results = vec![];
    let mut stderr = Vec::new();

    stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

    let output = String::from_utf8_lossy(&stderr);
    if quiet {
        assert!(output.is_empty(), "expected no output in quiet mode");
    } else {
        assert!(
            output.contains("Staging libraries to"),
            "expected progress message, got: {output}"
        );
    }
}

#[rstest]
fn stage_libraries_stages_build_results(
    #[from(staging_ctx)] staging_ctx_res: std::io::Result<StagingTestContext>,
) {
    use crate::builder::{library_extension, library_prefix};

    let staging_ctx = staging_ctx_res.expect("staging context should be created");

    let quiet_ctx = staging_ctx.with_quiet(true);
    let context = quiet_ctx.pipeline_context();
    let build_results = vec![
        create_mock_library(quiet_ctx.target_dir(), "whitaker_suite")
            .expect("mock library should be staged"),
    ];
    let mut stderr = Vec::new();

    let staging_path =
        stage_libraries(&context, &build_results, &mut stderr).expect("staging should succeed");

    // The staged filename must preserve crate and toolchain identity so
    // multi-toolchain installs do not collide.
    let staged_filename = format!(
        "{}whitaker_suite@nightly-2026-05-28{}",
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
fn stage_libraries_logs_installed_lints_when_not_quiet(
    #[from(staging_ctx)] staging_ctx_res: std::io::Result<StagingTestContext>,
) {
    let staging_ctx = staging_ctx_res.expect("staging context should be created");

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
    assert_bumpy_road_lint_in_staging_output!(experimental);
}
