//! The generated Windows wrapper prefers the installer beside it over `PATH`.
//!
//! A `.cmd` stub cannot stand in for the adjacent `whitaker-installer.exe`, so
//! this copies the real installer binary beside `whitaker.ps1` and puts a
//! stub on `PATH`. The stub exits with status 7 and prints `stub:` lines; the
//! real installer does neither, so their absence proves the adjacent
//! executable ran. The real installer is only ever invoked as
//! `run -- <args>`, which reads the bootstrap and never installs anything.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::dirs::BaseDirs;
use whitaker_installer::wrapper::generate_wrapper_scripts;

/// Status the `PATH` stub exits with; the real installer must not produce it.
const STUB_EXIT_CODE: i32 = 7;

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

/// Run `script` under PowerShell with `PATH` of `stub_dir` plus the system
/// directories that hold `powershell.exe` and `cmd.exe`.
fn run_powershell(script: &Path, args: &[&str], stub_dir: &Path) -> Output {
    let system_root = std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows");
    let system32 = Path::new(&system_root).join("System32");
    let powershell_dir = system32.join("WindowsPowerShell").join("v1.0");
    let path = std::env::join_paths([stub_dir.to_path_buf(), system32, powershell_dir])
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

#[test]
fn windows_whitaker_prefers_the_adjacent_installer_over_path() {
    let bin = TempDir::new().expect("create bin dir");
    let stub = TempDir::new().expect("create stub dir");
    let dirs = TempBaseDirs {
        bin: bin.path().to_path_buf(),
    };
    let result = generate_wrapper_scripts(&dirs).expect("generate wrapper scripts");
    std::fs::copy(
        env!("CARGO_BIN_EXE_whitaker-installer"),
        bin.path().join("whitaker-installer.exe"),
    )
    .expect("copy the real installer beside the wrapper");
    std::fs::write(
        stub.path().join("whitaker-installer.cmd"),
        STUB_INSTALLER_CMD,
    )
    .expect("write PATH stub installer");

    let output = run_powershell(&result.whitaker_path, &["--version"], stub.path());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.lines().any(|line| line.starts_with("stub:")),
        "the PATH stub ran instead of the adjacent installer: {stdout}"
    );
    assert_ne!(
        output.status.code(),
        Some(STUB_EXIT_CODE),
        "the PATH stub's exit status leaked through"
    );
}
