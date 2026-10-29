//! Sandbox and PowerShell stubs for the `verify-installed-bootstrap.ps1` test.
//!
//! The stubs mirror `tests/workflow_contracts/release_script_harness.py`: a
//! `.ps1` file per command is placed first on `PATH`, reads its configuration
//! from `FAKE_*` environment variables, and never touches the network or the
//! real hosts file. `FAKE_*` toggles make one stub violate one property.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Answers the dry run with the fixture's snapshot and driver locations.
const STUB_INSTALLER: &str = concat!(
    "Write-Output \"Clippy snapshot: $env:FAKE_SNAPSHOT\"\n",
    "Write-Output \"Dylint driver directory: $env:FAKE_DRIVER_DIR\"\n",
);

/// Reaches "GitHub" only while the scratch hosts file lacks a denial.
const STUB_GIT: &str = concat!(
    "if ($args[0] -ne 'ls-remote') { exit 0 }\n",
    "if ($env:FAKE_DENIAL_INEFFECTIVE -eq '1') { exit 0 }\n",
    "if ((Get-Content -Raw $env:FAKE_HOSTS) -match 'github\\.com') { exit 128 }\n",
    "exit 0\n",
);

/// Emulates `whitaker --all` against the `std::fs` fixture.
const STUB_WHITAKER: &str = concat!(
    "$snapshot = $env:FAKE_SNAPSHOT\n",
    "$driverDir = $env:FAKE_DRIVER_DIR\n",
    "function Say([string] $message) { [Console]::Error.WriteLine($message) }\n",
    "if (-not (Test-Path $snapshot)) {\n",
    "    if ($env:FAKE_SUCCEED_WITHOUT_SNAPSHOT -eq '1') { exit 0 }\n",
    "    if ($env:FAKE_SILENT_WITHOUT_SNAPSHOT -eq '1') {\n",
    "        Say 'error: snapshot missing'\n",
    "    } else {\n",
    "        Say 'error: snapshot missing; Whitaker never falls back to cloning public Clippy'\n",
    "    }\n",
    "    if ($env:FAKE_LINT_WITHOUT_SNAPSHOT -eq '1') { Say 'warning: no_std_fs_operations' }\n",
    "    exit 1\n",
    "}\n",
    "$driver = Join-Path (Join-Path $driverDir 'toolchain') 'dylint-driver'\n",
    "$built = \"$driverDir.built\"\n",
    "$lock = \"$driverDir.lock\"\n",
    "# FAKE_CONCURRENT_FAILS breaks any cold build after the first one ever made.\n",
    "if (-not (Test-Path $driver) -and $env:FAKE_CONCURRENT_FAILS -eq '1' -and (Test-Path $built)) {\n",
    "    Say 'error: the driver is being built by another process'\n",
    "    exit 1\n",
    "}\n",
    "if (-not (Test-Path $driver) -or $env:FAKE_REBUILD_ALWAYS -eq '1') {\n",
    "    try { $stream = [System.IO.File]::Open($lock, 'CreateNew') } catch { $stream = $null }\n",
    "    if ($stream) {\n",
    "        try {\n",
    "            New-Item -ItemType Directory -Force -Path (Split-Path $driver) | Out-Null\n",
    "            Set-Content -Path $built -Value 'built'\n",
    "            Start-Sleep -Milliseconds 500\n",
    "            $content = if ($env:FAKE_REBUILD_ALWAYS -eq '1') { \"$([DateTime]::UtcNow.Ticks)\" } else { 'v1' }\n",
    "            Set-Content -Path $driver -Value $content\n",
    "        } finally {\n",
    "            $stream.Dispose()\n",
    "            Remove-Item -Force $lock\n",
    "        }\n",
    "    } else {\n",
    "        while (Test-Path $lock) { Start-Sleep -Milliseconds 100 }\n",
    "    }\n",
    "}\n",
    "if ($env:FAKE_SUCCEED -eq '1') { exit 0 }\n",
    "if ($env:FAKE_NO_DIAGNOSTIC -eq '1') { Say 'error: crash' } else { Say 'error: no_std_fs_operations' }\n",
    "exit 1\n",
);

/// A scratch `PATH` of stubs, a hosts file, and an installed snapshot.
pub struct Sandbox {
    root: TempDir,
}

impl Default for Sandbox {
    /// Create a sandbox with stubs installed and the snapshot present.
    fn default() -> Self {
        let root = TempDir::new().expect("create the sandbox directory");
        let stubs = root.path().join("stubs");
        fs::create_dir(&stubs).expect("create the stubs directory");
        for (name, body) in [
            ("whitaker-installer.ps1", STUB_INSTALLER),
            ("git.ps1", STUB_GIT),
            ("whitaker.ps1", STUB_WHITAKER),
        ] {
            fs::write(stubs.join(name), body).expect("write a stub");
        }
        fs::write(root.path().join("hosts"), "127.0.0.1 localhost\n").expect("write hosts");
        fs::create_dir(root.path().join("snapshot")).expect("create the snapshot");
        Self { root }
    }
}

impl Sandbox {
    /// Return the scratch hosts file standing in for the system one.
    pub fn hosts(&self) -> PathBuf {
        self.root.path().join("hosts")
    }

    /// Return the installed snapshot directory.
    pub fn snapshot(&self) -> PathBuf {
        self.root.path().join("snapshot")
    }

    /// Run the real script with `FAKE_*` toggles set, returning its output.
    pub fn run(&self, toggles: &[(&str, &str)]) -> Output {
        let root = self.root.path();
        let script = repository_root()
            .join("scripts")
            .join("verify-installed-bootstrap.ps1");
        let path = std::env::join_paths(std::iter::once(root.join("stubs")).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .expect("join PATH entries");
        let mut command = Command::new("pwsh");
        command
            .args(["-NoProfile", "-NonInteractive", "-File"])
            .arg(script)
            .arg("-WorkDir")
            .arg(root.join("work"))
            .arg("-HostsFile")
            .arg(self.hosts())
            .env("PATH", path)
            .env("FAKE_HOSTS", self.hosts())
            .env("FAKE_SNAPSHOT", self.snapshot())
            .env("FAKE_DRIVER_DIR", root.join("driver"));
        for (key, value) in toggles {
            command.env(key, value);
        }
        command.output().expect("run pwsh")
    }
}

/// Return the repository root, the parent of this package's manifest directory.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .to_path_buf()
}
