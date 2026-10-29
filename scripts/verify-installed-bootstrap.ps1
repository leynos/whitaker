# verify-installed-bootstrap.ps1 - Prove an installed Whitaker lints without
# public Clippy on Windows, through the PowerShell wrapper.
#
# Usage:
#   pwsh scripts/verify-installed-bootstrap.ps1 -WorkDir <dir>
#
# The Windows counterpart of verify-installed-bootstrap.sh: deny github.com,
# confirm the denial with a canary, then require a cold and a warm run, two
# simultaneous cold runs, and a deleted snapshot to behave as specified. Data
# root relocation is covered on Linux, where XDG_DATA_HOME selects it.
param(
    [Parameter(Mandatory = $true)]
    [string] $WorkDir
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
        Add-Content -Path "$env:SystemRoot\System32\drivers\etc\hosts" -Value '0.0.0.0 github.com'
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

function Get-DriverDigest([string] $driverDir) {
    # Dylint persists the driver without an extension, on Windows too.
    $driver = Get-ChildItem -Recurse -File -Path $driverDir -Filter 'dylint-driver' |
        Select-Object -First 1
    if (-not $driver) { Fail "no driver was built under $driverDir" }
    return (Get-FileHash -Algorithm SHA256 $driver.FullName).Hash
}

$env:RUSTFLAGS = '-D warnings'
$snapshot = Get-DryRunValue 'Clippy snapshot'
$driverDir = Get-DryRunValue 'Dylint driver directory'
if (-not $snapshot -or -not (Test-Path $snapshot)) { Fail "the installed snapshot '$snapshot' is missing" }
if (-not $driverDir) { Fail 'the dry run did not name a driver directory' }

Deny-GitHub
$fixture = New-Fixture

Write-Output 'Cold run with an empty driver directory.'
if (Test-Path $driverDir) { Remove-Item -Recurse -Force $driverDir }
Assert-Diagnostic (Start-Whitaker $fixture (Join-Path $WorkDir 'cold.log')) (Join-Path $WorkDir 'cold.log')
$cold = Get-DriverDigest $driverDir

Write-Output 'Warm run reusing the driver.'
Assert-Diagnostic (Start-Whitaker $fixture (Join-Path $WorkDir 'warm.log')) (Join-Path $WorkDir 'warm.log')
if ((Get-DriverDigest $driverDir) -ne $cold) { Fail 'the warm run rebuilt the driver' }

Write-Output 'Two simultaneous cold runs.'
Remove-Item -Recurse -Force $driverDir
$first = Start-Whitaker $fixture (Join-Path $WorkDir 'concurrent-a.log')
$second = Start-Whitaker $fixture (Join-Path $WorkDir 'concurrent-b.log')
Assert-Diagnostic $first (Join-Path $WorkDir 'concurrent-a.log')
Assert-Diagnostic $second (Join-Path $WorkDir 'concurrent-b.log')

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

Write-Output 'The installed bootstrap lints without public Clippy.'
