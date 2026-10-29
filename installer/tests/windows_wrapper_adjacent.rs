//! The generated Windows wrapper prefers the installer beside it over `PATH`,
//! and forwards its arguments and status to that installer.
//!
//! A `.cmd` stub cannot stand in for the adjacent `whitaker-installer.exe`, so
//! the adjacent stand-in is this test binary itself, copied beside
//! `whitaker.ps1`. The wrapper invokes it as `whitaker-installer run -- ARGS`.
//! libtest treats everything after its options as name filters, so `run`
//! selects [`adjacent_stand_in_records_run`], which records the arguments and
//! exits with a sentinel status when the recording variable is set. A `.cmd`
//! stub on `PATH` exits with status 7 and prints `stub:` lines, so the test
//! also sees that it did not run.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::dirs::BaseDirs;
use whitaker_installer::wrapper::generate_wrapper_scripts;

/// A `PATH` stub that echoes each argument as `stub:<arg>` and exits with 7.
const STUB_INSTALLER_CMD: &str = concat!(
    "@echo off\r\n",
    ":loop\r\n",
    "if \"%~1\"==\"\" goto done\r\n",
    "echo stub:%~1\r\n",
    "shift\r\n",
    "goto loop\r\n",
    ":done\r\n",
    "exit /b 7\r\n",
);

/// Directory resolver that confines the wrapper output to a temporary bin dir.
struct TempBaseDirs {
    bin: PathBuf,
}

impl BaseDirs for TempBaseDirs {
    /// Return no home directory; the wrapper generator does not need one.
    fn home_dir(&self) -> Option<PathBuf> {
        None
    }

    /// Return the temporary bin directory.
    fn bin_dir(&self) -> Option<PathBuf> {
        Some(self.bin.clone())
    }

    /// Return no data directory; the wrapper generator does not need one.
    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        None
    }
}

/// Run `script` under PowerShell with `stub_dir` first on `PATH`, telling the
/// adjacent stand-in to record its arguments in `record`.
///
/// The test's own `PATH` follows the stub directory. Windows resolves a
/// program's DLLs through `PATH`, and the copied stand-in needs the ones Cargo
/// put there for this test process: without them it dies with
/// `STATUS_DLL_NOT_FOUND` before reading its arguments. That cannot weaken the
/// test, because the stub precedes every inherited entry and the adjacent
/// executable precedes `PATH` altogether.
fn run_powershell(script: &Path, args: &[&str], stub_dir: &Path, record: &Path) -> Output {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(stub_dir.to_path_buf()).chain(std::env::split_paths(&inherited)),
    )
    .expect("join PATH entries");
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .args(args)
        .env("PATH", path)
        .env(RECORD_ENV, record)
        .output()
        .expect("run wrapper")
}

/// Where the adjacent stand-in records its arguments, one per line.
const RECORD_ENV: &str = "WHITAKER_TEST_ADJACENT_RECORD";

/// The status the adjacent stand-in exits with; neither the PATH stub (7)
/// nor the wrapper's own lookup failure (127) produces it.
const SENTINEL_STATUS: i32 = 42;

/// The adjacent stand-in's entry point, a no-op in an ordinary test run.
///
/// Its name contains `run`, the first argument the wrapper forwards, so
/// libtest selects it when this binary is invoked as the adjacent installer.
/// No other test in this file may contain `run` or `--version`.
#[test]
fn adjacent_stand_in_records_run() {
    let Some(record) = std::env::var_os(RECORD_ENV) else {
        return;
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::fs::write(record, args.join("\n")).expect("record the forwarded arguments");
    std::process::exit(SENTINEL_STATUS);
}

#[test]
fn windows_whitaker_forwards_arguments_and_status_to_the_adjacent_installer() {
    let bin = TempDir::new().expect("create bin dir");
    let stub = TempDir::new().expect("create stub dir");
    let dirs = TempBaseDirs {
        bin: bin.path().to_path_buf(),
    };
    let result = generate_wrapper_scripts(&dirs).expect("generate wrapper scripts");
    let this_test = std::env::current_exe().expect("locate this test binary");
    std::fs::copy(&this_test, bin.path().join("whitaker-installer.exe"))
        .expect("copy the stand-in beside the wrapper");
    std::fs::write(
        stub.path().join("whitaker-installer.cmd"),
        STUB_INSTALLER_CMD,
    )
    .expect("write PATH stub installer");
    let record = bin.path().join("recorded-args.txt");

    let output = run_powershell(&result.whitaker_path, &["--version"], stub.path(), &record);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.lines().any(|line| line.starts_with("stub:")),
        "the PATH stub ran instead of the adjacent installer: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(SENTINEL_STATUS),
        "the adjacent installer's status is kept; stdout: {stdout} stderr: {stderr}"
    );
    let recorded = std::fs::read_to_string(&record).expect("the stand-in recorded its arguments");
    assert_eq!(
        recorded.lines().collect::<Vec<_>>(),
        ["run", "--", "--version"],
        "the wrapper forwards `run -- ARGS` to the adjacent installer"
    );
}
