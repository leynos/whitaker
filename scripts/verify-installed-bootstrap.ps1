# verify-installed-bootstrap.ps1 - Prove an installed Whitaker lints without
# public Clippy on Windows, through the PowerShell wrapper.
#
# Usage:
#   pwsh scripts/verify-installed-bootstrap.ps1 -WorkDir <dir> [-HostsFile <path>]
#
# -HostsFile names the file the github.com denial is appended to. It defaults
# to the system hosts file; tests pass a scratch file so they never alter it.
#
# The Windows counterpart of verify-installed-bootstrap.sh: deny github.com,
# confirm the denial with a canary, then require a cold and a warm run, two
# simultaneous cold runs, and a deleted snapshot to behave as specified. Data
# root relocation is covered on Linux, where XDG_DATA_HOME selects it.
param(
    [Parameter(Mandatory = $true)]
    [string] $WorkDir,

    [string] $HostsFile = "$env:SystemRoot\System32\drivers\etc\hosts"
)

$ErrorActionPreference = 'Stop'
$diagnostic = 'no_std_fs_operations'
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
$WorkDir = (Resolve-Path $WorkDir).Path

function Fail([string] $message) {
    [Console]::Error.WriteLine("verify-installed-bootstrap: $message")
    exit 1
}

function Get-DryRunValue([string] $label) {
    $lines = & whitaker-installer --dry-run --skip-deps 2>&1 | ForEach-Object { "$_" }
    foreach ($line in $lines) {
        if ($line.StartsWith("${label}: ")) { return $line.Substring($label.Length + 2) }
    }
    return ''
}

function Test-Canary {
    & git ls-remote https://github.com/rust-lang/rust-clippy HEAD *> $null
    return $LASTEXITCODE -eq 0
}

function Deny-GitHub {
    if (Test-Canary) {
        Add-Content -Path $HostsFile -Value '0.0.0.0 github.com'
    }
    if (Test-Canary) { Fail 'the github.com canary still succeeds; the denial did not take effect' }
    Write-Output 'github.com is unreachable; public Clippy cannot be read.'
}

function New-Fixture {
    $fixture = Join-Path $WorkDir 'fixture'
    if (Test-Path $fixture) { Remove-Item -Recurse -Force $fixture }
    New-Item -ItemType Directory -Force -Path (Join-Path $fixture 'src') | Out-Null
    Set-Content -Path (Join-Path $fixture 'Cargo.toml') -Value @'
[package]
name = "whitaker_bootstrap_fixture"
version = "0.1.0"
edition = "2021"

[workspace]
'@
    Set-Content -Path (Join-Path $fixture 'src/lib.rs') -Value @'
//! Deliberately bypasses the capability-based filesystem policy.

/// Read a file through ambient `std::fs`, which Whitaker forbids.
pub fn read_config() -> String {
    std::fs::read_to_string("config.toml").unwrap_or_default()
}
'@
    return $fixture
}

# Start the wrapper in the fixture; the caller waits for the process.
function Start-Whitaker([string] $fixture, [string] $log) {
    $wrapper = (Get-Command whitaker.ps1).Source
    return Start-Process -FilePath 'pwsh' -NoNewWindow -PassThru `
        -WorkingDirectory $fixture `
        -ArgumentList @('-NoProfile', '-File', $wrapper, '--all') `
        -RedirectStandardOutput "$log.out" -RedirectStandardError $log
}

function Assert-Diagnostic([System.Diagnostics.Process] $process, [string] $log) {
    $process.WaitForExit()
    $text = (Get-Content -Raw "$log.out") + (Get-Content -Raw $log)
    if ($process.ExitCode -eq 0) { Fail "whitaker succeeded on a std::fs violation; see $log" }
    if ($text -notmatch $diagnostic) {
        Fail "whitaker failed ($($process.ExitCode)) without a $diagnostic diagnostic; see $log"
    }
}

# `whitaker-installer run` prints this once a run has built the driver (see
# `BUILT_DRIVER_NOTICE` in installer/src/bootstrap/mod.rs).
$builtNotice = 'built the Dylint driver on first use'

# Count the build notices across both streams of the given logs.
function Get-BuildCount([string[]] $logs) {
    $count = 0
    foreach ($log in $logs) {
        foreach ($path in @("$log.out", $log)) {
            if (Test-Path -LiteralPath $path) {
                $count += @(Select-String -LiteralPath $path -SimpleMatch -Pattern $builtNotice).Count
            }
        }
    }
    return $count
}

function Get-DriverDigest([string] $driverDir) {
    # Dylint persists the driver without an extension, on Windows too.
    $driver = Get-ChildItem -Recurse -File -Path $driverDir -Filter 'dylint-driver' |
        Select-Object -First 1
    if (-not $driver) { Fail "no driver was built under $driverDir" }
    return (Get-FileHash -Algorithm SHA256 $driver.FullName).Hash
}

# Require exactly $expected build notices across $logs.
function Assert-BuildCount([string[]] $logs, [int] $expected, [string] $message) {
    $builds = Get-BuildCount $logs
    if ($builds -ne $expected) { Fail ($message -f $builds) }
}

# Read the installed snapshot and driver locations from the installer's dry run.
function Get-Installation {
    $snapshot = Get-DryRunValue 'Clippy snapshot'
    $driverDir = Get-DryRunValue 'Dylint driver directory'
    if (-not $snapshot -or -not (Test-Path $snapshot)) { Fail "the installed snapshot '$snapshot' is missing" }
    if (-not $driverDir) { Fail 'the dry run did not name a driver directory' }
    return [pscustomobject]@{ Snapshot = $snapshot; DriverDir = $driverDir }
}

# Dylint reuses a persisted driver only if `dylint-driver -V` succeeds, and
# rebuilds it otherwise, silently. Run that probe by hand, as it is and with
# the toolchain's own bin directory first on PATH (where the compiler's DLLs
# live), and say whether a `.exe`-suffixed sibling exists, so a failed probe
# names its cause.
function Write-DriverProbe([string] $driverDir) {
    $driver = Get-ChildItem -Recurse -File -Path $driverDir -Filter 'dylint-driver' |
        Select-Object -First 1
    if (-not $driver) { return }
    # Dylint spawns the probe through Rust's `std::process::Command`, so the
    # probe is a small Rust program doing the same, not a PowerShell call
    # (PowerShell refuses to run an extensionless file at all).
    $toolchain = $driver.Directory.Name
    $sysroot = (& rustc "+$toolchain" --print sysroot 2>&1 | Select-Object -First 1)
    [Console]::Error.WriteLine("sysroot: $sysroot; dylint-driver.exe present: $(Test-Path ($driver.FullName + '.exe'))")
    $probeDir = Join-Path $WorkDir 'probe'
    New-Item -ItemType Directory -Force -Path $probeDir | Out-Null
    $source = Join-Path $probeDir 'probe.rs'
    Set-Content -Path $source -Value @'
use std::{env, process::Command};

fn main() {
    let path = env::args().nth(1).expect("driver path");
    let result = Command::new(&path).arg("-V").output();
    match result {
        Ok(out) => println!(
            "status={:?} stdout={:?} stderr={:?}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(error) => println!("spawn error: {error}"),
    }
}
'@
    & rustc --edition 2021 -o (Join-Path $probeDir 'probe.exe') $source 2>&1 | Out-Null
    $probe = Join-Path $probeDir 'probe.exe'
    $saved = $env:PATH
    foreach ($label in @('as is', 'with the toolchain bin first')) {
        $env:PATH = if ($label -eq 'as is') { $saved } else { "$sysroot\bin;$saved" }
        $line = (& $probe $driver.FullName 2>&1 | Out-String).Trim()
        [Console]::Error.WriteLine("probe ($label): $line")
    }
    $env:PATH = $saved
}

# Print what a failed warm run left behind: every driver file with its hash and
# timestamps, then the warm run's own output. The runner's temp directory is
# gone once the job ends, so this is the evidence a red run carries.
function Write-WarmDiagnostics([string] $driverDir, [string] $coldDigest, [string] $warmLog) {
    [Console]::Error.WriteLine("cold digest: $coldDigest")
    [Console]::Error.WriteLine("driver directory: $driverDir")
    Get-ChildItem -Recurse -Force -File -Path $driverDir | ForEach-Object {
        $hash = (Get-FileHash -Algorithm SHA256 $_.FullName).Hash
        [Console]::Error.WriteLine(
            "  $($_.FullName)  length=$($_.Length) sha256=$hash " +
            "created=$($_.CreationTimeUtc.ToString('o')) written=$($_.LastWriteTimeUtc.ToString('o'))")
    }
    Write-DriverProbe $driverDir
    foreach ($path in @("$warmLog.out", $warmLog)) {
        if (Test-Path -LiteralPath $path) {
            [Console]::Error.WriteLine("--- $path")
            Get-Content -LiteralPath $path | ForEach-Object { [Console]::Error.WriteLine($_) }
        }
    }
}

# A cold run builds the driver once; a warm run then reuses it untouched.
function Test-ColdThenWarm([string] $fixture, [string] $driverDir) {
    Write-Output 'Cold run with an empty driver directory.'
    if (Test-Path $driverDir) { Remove-Item -Recurse -Force $driverDir }
    $coldLog = Join-Path $WorkDir 'cold.log'
    Assert-Diagnostic (Start-Whitaker $fixture $coldLog) $coldLog
    $cold = Get-DriverDigest $driverDir
    Assert-BuildCount @($coldLog) 1 "the cold run did not report building the driver once (it reported {0}); see $coldLog"

    Write-Output 'Warm run reusing the driver.'
    $warmLog = Join-Path $WorkDir 'warm.log'
    Assert-Diagnostic (Start-Whitaker $fixture $warmLog) $warmLog
    # The notice also catches a rebuild that happens to write identical bytes.
    if ((Get-DriverDigest $driverDir) -ne $cold) {
        Write-WarmDiagnostics $driverDir $cold $warmLog
        Fail 'the warm run rebuilt the driver'
    }
    Assert-BuildCount @($warmLog) 0 'the warm run rebuilt the driver'
}

# Two simultaneous cold runs both lint, and only one of them builds.
function Test-ConcurrentColdRuns([string] $fixture, [string] $driverDir) {
    Write-Output 'Two simultaneous cold runs.'
    Remove-Item -Recurse -Force $driverDir
    $logs = @((Join-Path $WorkDir 'concurrent-a.log'), (Join-Path $WorkDir 'concurrent-b.log'))
    $first = Start-Whitaker $fixture $logs[0]
    $second = Start-Whitaker $fixture $logs[1]
    Assert-Diagnostic $first $logs[0]
    Assert-Diagnostic $second $logs[1]
    Assert-BuildCount $logs 1 'the simultaneous cold runs built the driver {0} times'
}

# A deleted snapshot fails clearly, before any lint runs, and is restored.
function Test-DeletedSnapshot([string] $fixture, [string] $snapshot) {
    Write-Output 'Deleted snapshot generation.'
    Rename-Item -Path $snapshot -NewName ((Split-Path -Leaf $snapshot) + '.removed')
    $log = Join-Path $WorkDir 'deleted.log'
    $process = Start-Whitaker $fixture $log
    $process.WaitForExit()
    Rename-Item -Path "$snapshot.removed" -NewName (Split-Path -Leaf $snapshot)
    $text = (Get-Content -Raw "$log.out") + (Get-Content -Raw $log)
    if ($process.ExitCode -eq 0) { Fail 'whitaker ran without its Clippy snapshot' }
    if ($text -notmatch 'never falls back to cloning public Clippy') {
        Fail "a missing snapshot did not fail clearly; see $log"
    }
    if ($text -match $diagnostic) { Fail 'lints ran although the snapshot was missing' }
}

# Run every check in order; any failure exits through Fail.
function Invoke-Verification {
    $env:RUSTFLAGS = '-D warnings'
    $installation = Get-Installation
    Deny-GitHub
    $fixture = New-Fixture
    Test-ColdThenWarm $fixture $installation.DriverDir
    Test-ConcurrentColdRuns $fixture $installation.DriverDir
    Test-DeletedSnapshot $fixture $installation.Snapshot
    Write-Output 'The installed bootstrap lints without public Clippy.'
}

Invoke-Verification

# The canary's `git ls-remote` fails by design and leaves its exit code in
# `$LASTEXITCODE`, which `pwsh -command` would report as the script's status.
exit 0
