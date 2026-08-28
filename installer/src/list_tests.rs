//! Tests for the `list` command handler and its path resolution helpers.

use super::*;
use rstest::{fixture, rstest};
use std::fs;
use tempfile::TempDir;

// -------------------------------------------------------------------------
// Fixtures
// -------------------------------------------------------------------------

/// A temporary directory converted to a UTF-8 path for test isolation.
struct TempTarget {
    _temp: TempDir,
    path: Utf8PathBuf,
}

#[fixture]
fn temp_target() -> TempTarget {
    let temp = TempDir::new().expect("failed to create temp dir");
    let path = Utf8PathBuf::try_from(temp.path().to_owned()).expect("non-UTF8 temp path");
    TempTarget { _temp: temp, path }
}

/// A Write implementation that always fails, for testing error paths.
struct FailingWriter;

impl std::io::Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("simulated write failure"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("simulated flush failure"))
    }
}

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum MockLibraryKind {
    Local,
    Prebuilt { target: &'static str },
}

impl MockLibraryKind {
    fn library_dir(&self, target_dir: &Utf8Path, toolchain: &str) -> Utf8PathBuf {
        match self {
            Self::Local => target_dir.join(toolchain).join("release"),
            Self::Prebuilt { target } => target_dir.join(toolchain).join(target).join("lib"),
        }
    }

    fn content(&self) -> &'static [u8] {
        match self {
            Self::Local => b"mock library",
            Self::Prebuilt { .. } => b"mock prebuilt library",
        }
    }
}

fn create_mock_library_internal(target_dir: &Utf8Path, toolchain: &str, kind: MockLibraryKind) {
    use crate::builder::{library_extension, library_prefix};

    let lib_dir = kind.library_dir(target_dir, toolchain);
    fs::create_dir_all(&lib_dir).expect("failed to create target library directory");

    let filename = format!(
        "{}whitaker_suite@{toolchain}{}",
        library_prefix(),
        library_extension()
    );

    let error_msg = match kind {
        MockLibraryKind::Local => "failed to create mock library",
        MockLibraryKind::Prebuilt { .. } => "failed to create prebuilt mock library",
    };
    fs::write(lib_dir.join(filename), kind.content()).expect(error_msg);
}

/// Helper to create a mock installed library in the target directory for tests.
fn create_mock_library(target_dir: &Utf8Path, toolchain: &str) {
    create_mock_library_internal(target_dir, toolchain, MockLibraryKind::Local);
}

fn create_mock_prebuilt_library(target_dir: &Utf8Path, toolchain: &str, target: &'static str) {
    create_mock_library_internal(target_dir, toolchain, MockLibraryKind::Prebuilt { target });
}

// -------------------------------------------------------------------------
// run_list tests
// -------------------------------------------------------------------------

#[rstest]
fn run_list_outputs_human_readable_format(temp_target: TempTarget) {
    let args = ListArgs {
        json: false,
        target_dir: Some(temp_target.path.clone()),
    };
    let mut stdout = Vec::new();

    let result = run_list_with(&args, &mut stdout, || None);

    assert!(result.is_ok(), "expected success, got: {result:?}");
    let output = String::from_utf8_lossy(&stdout);
    assert!(output.contains("No lints installed"), "got: {output}");
}

#[rstest]
#[case::json_format(true, &["toolchains", "\"active\""])]
#[case::human_format(false, &["nightly-2026-05-28", "whitaker_suite"])]
fn run_list_with_installed_library_includes_expected_output(
    temp_target: TempTarget,
    #[case] json: bool,
    #[case] expected: &[&str],
) {
    create_mock_library(&temp_target.path, "nightly-2026-05-28");
    let args = ListArgs {
        json,
        target_dir: Some(temp_target.path.clone()),
    };
    let mut stdout = Vec::new();

    let result = run_list_with(&args, &mut stdout, || Some("nightly-2026-05-28".to_owned()));

    assert!(result.is_ok(), "expected success, got: {result:?}");
    let output = String::from_utf8_lossy(&stdout);
    for needle in expected {
        assert!(
            output.contains(needle),
            "expected '{needle}' in output: {output}"
        );
    }
}

#[rstest]
fn run_list_finds_prebuilt_layout_libraries(temp_target: TempTarget) {
    create_mock_prebuilt_library(
        &temp_target.path,
        "nightly-2026-05-28",
        "x86_64-unknown-linux-gnu",
    );
    let args = ListArgs {
        json: false,
        target_dir: Some(temp_target.path.clone()),
    };
    let mut stdout = Vec::new();

    let result = run_list_with(&args, &mut stdout, || Some("nightly-2026-05-28".to_owned()));

    assert!(result.is_ok(), "expected success, got: {result:?}");
    let output = String::from_utf8_lossy(&stdout);
    assert!(output.contains("nightly-2026-05-28"), "got: {output}");
    assert!(output.contains("whitaker_suite"), "got: {output}");
}

#[rstest]
fn run_list_returns_write_failed_on_stdout_error(temp_target: TempTarget) {
    let args = ListArgs {
        json: false,
        target_dir: Some(temp_target.path.clone()),
    };
    let mut failing_stdout = FailingWriter;

    let result = run_list_with(&args, &mut failing_stdout, || None);

    let err = result.expect_err("expected error on write failure");
    assert!(
        matches!(err, InstallerError::WriteFailed { .. }),
        "expected WriteFailed error, got: {err:?}"
    );
}

// -------------------------------------------------------------------------
// detect_active_toolchain_in tests
// -------------------------------------------------------------------------

#[rstest]
fn detect_active_toolchain_in_returns_none_when_no_toolchain_file(temp_target: TempTarget) {
    let result = detect_active_toolchain_in(&temp_target.path);
    assert!(
        result.is_none(),
        "expected None for directory without rust-toolchain.toml"
    );
}

#[rstest]
fn detect_active_toolchain_in_returns_channel_when_toolchain_file_exists(temp_target: TempTarget) {
    // Create a rust-toolchain.toml file
    let toolchain_content = r#"[toolchain]
channel = "nightly-2026-05-28"
"#;
    fs::write(
        temp_target.path.join("rust-toolchain.toml"),
        toolchain_content,
    )
    .expect("failed to write rust-toolchain.toml");

    let result = detect_active_toolchain_in(&temp_target.path);

    assert_eq!(result, Some("nightly-2026-05-28".to_owned()));
}

// -------------------------------------------------------------------------
// determine_target_dir tests
// -------------------------------------------------------------------------

#[rstest]
fn determine_target_dir_returns_cli_value_when_provided(temp_target: TempTarget) {
    let result = determine_target_dir_with(Some(&temp_target.path), || None);

    assert!(result.is_ok(), "expected success, got: {result:?}");
    assert_eq!(result.expect("already checked"), temp_target.path);
}

#[rstest]
fn determine_target_dir_falls_back_to_default_when_cli_is_none(temp_target: TempTarget) {
    let default_path = temp_target.path.clone();

    let result = determine_target_dir_with(None, || Some(default_path.clone()));

    assert!(result.is_ok(), "expected success, got: {result:?}");
    assert_eq!(result.expect("already checked"), default_path);
}

#[test]
fn determine_target_dir_returns_error_when_no_default_available() {
    let result = determine_target_dir_with(None, || None);

    let err = result.expect_err("expected error when no default");
    assert!(
        matches!(err, InstallerError::StagingFailed { .. }),
        "expected StagingFailed error, got: {err:?}"
    );
}

#[rstest]
fn determine_target_dir_prefers_cli_over_default(temp_target: TempTarget) {
    let cli_path = temp_target.path.clone();
    let default_path = temp_target.path.join("should_not_be_used");

    let result = determine_target_dir_with(Some(&cli_path), || Some(default_path));

    assert!(result.is_ok(), "expected success, got: {result:?}");
    assert_eq!(result.expect("already checked"), cli_path);
}
