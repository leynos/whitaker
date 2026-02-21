//! Tests for the installer CLI entrypoint.

use super::*;
use rstest::rstest;
use std::path::PathBuf;
use std::time::Duration;
use whitaker_installer::cli::InstallArgs;
use whitaker_installer::dirs::BaseDirs;
use whitaker_installer::test_utils::*;

#[test]
fn exit_code_for_run_result_returns_zero_on_success() {
    let mut stderr = Vec::new();
    let exit_code = exit_code_for_run_result(Ok(()), &mut stderr);
    assert_eq!(exit_code, 0);
    assert!(stderr.is_empty());
}

#[test]
fn exit_code_for_run_result_prints_error_and_returns_one() {
    let err = InstallerError::LintCrateNotFound {
        name: CrateName::from("nonexistent_lint"),
    };

    let mut stderr = Vec::new();
    let exit_code = exit_code_for_run_result(Err(err), &mut stderr);
    assert_eq!(exit_code, 1);

    let stderr_text = String::from_utf8(stderr).expect("stderr was not UTF-8");
    assert!(stderr_text.contains("lint crate nonexistent_lint not found"));
}

#[rstest]
#[case::default_suite_only(InstallArgs::default(), false, true)]
#[case::individual_lints(
    InstallArgs { individual_lints: true, ..InstallArgs::default() },
    true,
    false
)]
fn resolve_requested_crates_respects_individual_lints_flag(
    #[case] args: InstallArgs,
    #[case] expect_lint: bool,
    #[case] expect_suite: bool,
) {
    let crates = resolve_requested_crates(&args).expect("expected crate resolution to succeed");
    assert_eq!(
        crates.contains(&CrateName::from("module_max_lines")),
        expect_lint
    );
    assert_eq!(
        crates.contains(&CrateName::from("whitaker_suite")),
        expect_suite
    );
}

#[test]
fn resolve_requested_crates_returns_specific_lints_when_provided() {
    let args = InstallArgs {
        lint: vec!["module_max_lines".to_owned()],
        ..InstallArgs::default()
    };

    let crates = resolve_requested_crates(&args).expect("expected crate resolution to succeed");
    assert_eq!(crates, vec![CrateName::from("module_max_lines")]);
}

#[test]
fn resolve_requested_crates_rejects_unknown_lints() {
    let args = InstallArgs {
        lint: vec!["nonexistent_lint".to_owned()],
        ..InstallArgs::default()
    };

    let err = resolve_requested_crates(&args).expect_err("expected crate resolution to fail");
    assert!(matches!(
        err,
        InstallerError::LintCrateNotFound { name } if name == CrateName::from("nonexistent_lint")
    ));
}

#[test]
fn ensure_dylint_tools_skips_install_when_installed() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: Ok(success_output()),
        },
    ]);

    let mut stderr = Vec::new();
    let result = ensure_dylint_tools_with_executor(&executor, false, &mut stderr);

    assert!(result.is_ok());
    assert!(stderr.is_empty());
    executor.assert_finished();
}

#[rstest]
#[case::logs_when_not_quiet(
    false,
    "Installing required Dylint tools...\n",
    "Dylint tools installed successfully.\n\n"
)]
#[case::quiet_suppresses_output(true, "", "")]
fn ensure_dylint_tools_installs_missing_tools(
    #[case] quiet: bool,
    #[case] expected_start: &str,
    #[case] expected_end: &str,
) {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: Ok(failure_output("missing cargo-dylint")),
        },
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: Ok(failure_output("missing dylint-link")),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "cargo-dylint"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "dylint-link"],
            result: Ok(success_output()),
        },
    ]);

    let mut stderr = Vec::new();
    let result = ensure_dylint_tools_with_executor(&executor, quiet, &mut stderr);

    assert!(result.is_ok());
    let stderr_text = String::from_utf8(stderr).expect("stderr was not UTF-8");
    if quiet {
        assert!(
            stderr_text.is_empty(),
            "expected stderr to be empty when quiet, got {stderr_text:?}"
        );
    } else {
        assert!(
            stderr_text.starts_with(expected_start),
            "expected stderr to start with {expected_start:?}, got {stderr_text:?}"
        );
        assert!(
            stderr_text.ends_with(expected_end),
            "expected stderr to end with {expected_end:?}, got {stderr_text:?}"
        );
    }
    executor.assert_finished();
}

#[test]
fn ensure_dylint_tools_propagates_install_failures() {
    let executor = StubExecutor::new(vec![
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: Ok(failure_output("missing cargo-dylint")),
        },
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: Ok(success_output()),
        },
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "-y", "cargo-dylint"],
            result: Ok(failure_output("install failed")),
        },
    ]);

    let mut stderr = Vec::new();
    let err = ensure_dylint_tools_with_executor(&executor, false, &mut stderr)
        .expect_err("expected install failure");

    assert!(matches!(
        err,
        InstallerError::DependencyInstall { tool, message }
            if tool == "cargo-dylint" && message == "install failed"
    ));
    executor.assert_finished();
}

#[derive(Debug, Clone)]
struct TestBaseDirs {
    home_dir: Option<PathBuf>,
    bin_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
}

impl BaseDirs for TestBaseDirs {
    fn home_dir(&self) -> Option<PathBuf> {
        self.home_dir.clone()
    }

    fn bin_dir(&self) -> Option<PathBuf> {
        self.bin_dir.clone()
    }

    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        self.data_dir.clone()
    }
}

#[test]
fn write_install_metrics_prints_summary_and_persists_metrics() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let dirs = TestBaseDirs {
        home_dir: Some(temp_dir.path().to_path_buf()),
        bin_dir: Some(temp_dir.path().join("bin")),
        data_dir: Some(temp_dir.path().to_path_buf()),
    };

    let mut stderr = Vec::new();
    let context = MetricsWriteContext {
        quiet: false,
        dirs: &dirs,
        install_mode: InstallMode::Download,
        elapsed: Duration::from_millis(1250),
    };
    write_install_metrics(&context, &mut stderr);

    let stderr_text = String::from_utf8(stderr).expect("stderr UTF-8");
    assert!(stderr_text.contains("Install metrics:"));
    assert!(stderr_text.contains("download 1/1 (100.0%)"));
    assert!(stderr_text.contains("build 0/1 (0.0%)"));
    assert!(stderr_text.contains("total installation time 1.250s"));

    let metrics_path = temp_dir.path().join("metrics").join("install_metrics.json");
    assert!(
        metrics_path.exists(),
        "expected metrics file at {:?}",
        metrics_path
    );
}

#[test]
fn write_install_metrics_warns_when_recording_fails() {
    let dirs = TestBaseDirs {
        home_dir: None,
        bin_dir: None,
        data_dir: None,
    };

    let mut stderr = Vec::new();
    let context = MetricsWriteContext {
        quiet: false,
        dirs: &dirs,
        install_mode: InstallMode::Build,
        elapsed: Duration::from_secs(1),
    };
    write_install_metrics(&context, &mut stderr);

    let stderr_text = String::from_utf8(stderr).expect("stderr UTF-8");
    assert!(stderr_text.contains("Warning: could not record install metrics"));
}

#[test]
fn write_install_metrics_suppresses_output_in_quiet_mode() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let dirs = TestBaseDirs {
        home_dir: Some(temp_dir.path().to_path_buf()),
        bin_dir: Some(temp_dir.path().join("bin")),
        data_dir: Some(temp_dir.path().to_path_buf()),
    };

    let mut stderr = Vec::new();
    let context = MetricsWriteContext {
        quiet: true,
        dirs: &dirs,
        install_mode: InstallMode::Build,
        elapsed: Duration::from_millis(500),
    };
    write_install_metrics(&context, &mut stderr);

    assert!(stderr.is_empty(), "expected no stderr output in quiet mode");
}
