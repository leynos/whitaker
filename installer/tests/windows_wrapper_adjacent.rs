//! The generated Windows wrapper prefers the installer beside it over `PATH`.
//!
//! A `.cmd` stub cannot stand in for the adjacent `whitaker-installer.exe`, so
//! this copies a real executable from this package beside `whitaker.ps1` and
//! puts a stub on `PATH`. The stub exits with status 7 and prints `stub:`
//! lines; the adjacent executable answers with its own usage error, so the
//! test sees which one ran.
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

/// Run `script` under PowerShell with `stub_dir` first on `PATH`.
///
/// The test's own `PATH` follows the stub directory. Windows resolves a
/// program's DLLs through `PATH`, and the copied stand-in needs the ones Cargo
/// put there for this test process: without them it dies with
/// `STATUS_DLL_NOT_FOUND` before reading its arguments. That cannot weaken the
/// test, because the stub precedes every inherited entry and the adjacent
/// executable precedes `PATH` altogether.
fn run_powershell(script: &Path, args: &[&str], stub_dir: &Path) -> Output {
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
        .output()
        .expect("run wrapper")
}

/// The adjacent stand-in is the package's `whitaker-clippy-snapshot`
/// binary: invoked as `whitaker-installer run -- ...`, clap refuses the
/// unknown `run` subcommand with a fixed message and status 2, whatever the
/// host has installed. Neither the PATH stub, the wrapper's own lookup
/// failure (status 127), nor the real installer (which has `run`) can produce
/// that, so it is positive evidence the adjacent executable ran.
#[test]
fn windows_whitaker_prefers_the_adjacent_installer_over_path() {
    let bin = TempDir::new().expect("create bin dir");
    let stub = TempDir::new().expect("create stub dir");
    let dirs = TempBaseDirs {
        bin: bin.path().to_path_buf(),
    };
    let result = generate_wrapper_scripts(&dirs).expect("generate wrapper scripts");
    std::fs::copy(
        env!("CARGO_BIN_EXE_whitaker-clippy-snapshot"),
        bin.path().join("whitaker-installer.exe"),
    )
    .expect("copy a stand-in installer beside the wrapper");
    std::fs::write(
        stub.path().join("whitaker-installer.cmd"),
        STUB_INSTALLER_CMD,
    )
    .expect("write PATH stub installer");

    let output = run_powershell(&result.whitaker_path, &["--version"], stub.path());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.lines().any(|line| line.starts_with("stub:")),
        "the PATH stub ran instead of the adjacent installer: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "the adjacent stand-in's clap usage status is kept; stderr: {stderr}"
    );
    assert!(
        stderr.contains("unrecognized subcommand 'run'"),
        "the adjacent stand-in received `run`: {stderr}"
    );
}
