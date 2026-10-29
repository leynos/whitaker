//! The generated scripts, executed against a stub `whitaker-installer`.

use super::*;
#[cfg(any(unix, windows))]
use std::process::Command;
use tempfile::TempDir;

/// A stub installer that echoes its arguments, one per line, and exits with
/// `STUB_EXIT`.
#[cfg(unix)]
const STUB_INSTALLER: &str = r#"#!/usr/bin/env bash
printf 'stub:%s\n' "$@"
if [[ "${2:-}" == "--" && "${3:-}" == list ]]; then
    printf 'whitaker_suite    nightly    /lib\nother_suite   nightly   /lib\n'
fi
exit "${STUB_EXIT:-0}"
"#;

#[cfg(unix)]
fn write_executable(path: &Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, contents).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// Generate the scripts into a temporary bin directory.
#[cfg(unix)]
fn scripts() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp = TempDir::new().expect("create temp dir");
    let (whitaker, whitaker_ls) = generate_unix_scripts(temp.path()).expect("generate scripts");
    (temp, whitaker, whitaker_ls)
}

/// Run `script` with `args` and a `PATH` of only `path_dir` plus the system.
#[cfg(unix)]
fn run(script: &Path, args: &[&str], path_dir: Option<&Path>) -> std::process::Output {
    let system_path = "/usr/bin:/bin";
    let path = path_dir.map_or_else(
        || system_path.to_owned(),
        |dir| format!("{}:{system_path}", dir.display()),
    );
    Command::new(script)
        .args(args)
        .env("PATH", path)
        .env("STUB_EXIT", "7")
        .output()
        .expect("run wrapper")
}

#[cfg(unix)]
#[test]
fn scripts_are_executable_and_embed_no_paths() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, whitaker, whitaker_ls) = scripts();

    for script in [&whitaker, &whitaker_ls] {
        let mode = std::fs::metadata(script)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
        let content = std::fs::read_to_string(script).expect("read script");
        assert!(content.contains("set -euo pipefail"));
        assert!(content.contains("run --"));
        assert!(
            !content.contains("DYLINT_"),
            "the dispatcher resolves every path"
        );
    }
}

#[cfg(unix)]
#[test]
fn whitaker_prefers_the_installer_beside_it_and_keeps_arguments_and_status() {
    let (temp, whitaker, _) = scripts();
    write_executable(&temp.path().join("whitaker-installer"), STUB_INSTALLER);

    let output = run(&whitaker, &["--all", "a spaced argument"], None);

    assert_eq!(
        output.status.code(),
        Some(7),
        "the exit status passes through"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "stub:run\nstub:--\nstub:--all\nstub:a spaced argument\n"
    );
}

#[cfg(unix)]
#[test]
fn whitaker_falls_back_to_the_installer_on_path() {
    let (_temp, whitaker, _) = scripts();
    let elsewhere = TempDir::new().expect("create temp dir");
    write_executable(&elsewhere.path().join("whitaker-installer"), STUB_INSTALLER);

    let output = run(&whitaker, &["--all"], Some(elsewhere.path()));

    assert_eq!(output.status.code(), Some(7));
    assert!(String::from_utf8_lossy(&output.stdout).contains("stub:--all"));
}

#[cfg(unix)]
#[test]
fn whitaker_reports_a_missing_installer() {
    let (_temp, whitaker, _) = scripts();

    let output = run(&whitaker, &["--all"], None);

    assert_eq!(output.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo binstall whitaker-installer"));
}

#[cfg(unix)]
#[test]
fn whitaker_ls_filters_to_the_suite_and_keeps_the_status() {
    let (temp, _, whitaker_ls) = scripts();
    write_executable(&temp.path().join("whitaker-installer"), STUB_INSTALLER);

    let output = run(&whitaker_ls, &[], None);

    assert_eq!(
        output.status.code(),
        Some(7),
        "pipefail keeps the dispatcher's status"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "whitaker_suite    nightly    /lib\n"
    );
}

/// A batch stub installer that echoes its arguments, one per line, and exits
/// with `STUB_EXIT`. PowerShell's `Get-Command -CommandType Application`
/// resolves `.cmd` files on `PATH`, so this stands in for the real `.exe`.
#[cfg(windows)]
const STUB_INSTALLER_CMD: &str = concat!(
    "@echo off\r\n",
    "if \"%~2\"==\"--\" if \"%~3\"==\"list\" (\r\n",
    "    echo whitaker_suite    nightly    /lib\r\n",
    "    echo other_suite   nightly   /lib\r\n",
    ")\r\n",
    ":loop\r\n",
    "if \"%~1\"==\"\" goto done\r\n",
    "echo stub:%~1\r\n",
    "shift\r\n",
    "goto loop\r\n",
    ":done\r\n",
    "exit /b %STUB_EXIT%\r\n",
);

/// Generate the PowerShell scripts into a temporary bin directory.
#[cfg(windows)]
fn windows_scripts() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp = TempDir::new().expect("create temp dir");
    let (whitaker, whitaker_ls) = generate_windows_scripts(temp.path()).expect("generate scripts");
    (temp, whitaker, whitaker_ls)
}

/// Create a directory holding a `whitaker-installer.cmd` stub.
#[cfg(windows)]
fn stub_installer_dir() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    std::fs::write(
        dir.path().join("whitaker-installer.cmd"),
        STUB_INSTALLER_CMD,
    )
    .expect("write stub installer");
    dir
}

/// Run `script` under PowerShell with a `PATH` of only `path_dir` plus the
/// directories that hold `powershell.exe` and `cmd.exe`.
#[cfg(windows)]
fn run_powershell(script: &Path, args: &[&str], path_dir: Option<&Path>) -> std::process::Output {
    let system_root = std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows");
    let system32 = Path::new(&system_root).join("System32");
    let powershell_dir = system32.join("WindowsPowerShell").join("v1.0");
    let dirs = path_dir
        .map(Path::to_path_buf)
        .into_iter()
        .chain([system32, powershell_dir]);
    let path = std::env::join_paths(dirs).expect("join PATH entries");
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
        .env("STUB_EXIT", "7")
        .output()
        .expect("run wrapper")
}

/// Split captured stdout into lines, tolerating CRLF line endings.
#[cfg(windows)]
fn stdout_lines(output: &std::process::Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

#[cfg(windows)]
#[test]
fn windows_scripts_forward_to_the_dispatcher_and_embed_no_paths() {
    let (_temp, whitaker, whitaker_ls) = windows_scripts();

    for script in [&whitaker, &whitaker_ls] {
        let content = std::fs::read_to_string(script).expect("read script");
        assert!(content.contains("run --"));
        assert!(
            !content.contains("DYLINT_"),
            "the dispatcher resolves every path"
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_whitaker_finds_the_installer_on_path_and_keeps_arguments_and_status() {
    let (_temp, whitaker, _) = windows_scripts();
    let stub = stub_installer_dir();

    let output = run_powershell(
        &whitaker,
        &["--all", "a spaced argument"],
        Some(stub.path()),
    );

    assert_eq!(
        output.status.code(),
        Some(7),
        "the exit status passes through"
    );
    assert_eq!(
        stdout_lines(&output),
        [
            "stub:run",
            "stub:--",
            "stub:--all",
            "stub:a spaced argument"
        ]
    );
}

#[cfg(windows)]
#[test]
fn windows_whitaker_reports_a_missing_installer() {
    let (_temp, whitaker, _) = windows_scripts();

    let output = run_powershell(&whitaker, &["--all"], None);

    assert_eq!(output.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo binstall whitaker-installer"));
}

#[cfg(windows)]
#[test]
fn windows_whitaker_ls_filters_to_the_suite_and_keeps_the_status() {
    let (_temp, _, whitaker_ls) = windows_scripts();
    let stub = stub_installer_dir();

    let output = run_powershell(&whitaker_ls, &[], Some(stub.path()));

    assert_eq!(
        output.status.code(),
        Some(7),
        "the dispatcher's status is kept"
    );
    assert_eq!(stdout_lines(&output), ["whitaker_suite    nightly    /lib"]);
}

#[test]
fn is_directory_in_path_returns_false_for_random_dir() {
    let temp = TempDir::new().expect("failed to create temp dir");
    assert!(!is_directory_in_path(temp.path()));
}

#[test]
fn path_instructions_contains_directory() {
    let dir = std::path::PathBuf::from("/test/bin");
    let instructions = path_instructions(&dir);
    assert!(instructions.contains("/test/bin"));
}

/// Regression: the template once doubled its backslashes inside a raw string,
/// so PowerShell matched a literal `\s` and `whitaker-ls` printed nothing.
#[test]
fn windows_ls_pattern_uses_regex_whitespace_classes() {
    let script = windows_ls_script();
    let pattern_line = script
        .lines()
        .find(|line| line.contains("-match"))
        .expect("the script filters with -match");

    assert_eq!(
        pattern_line.trim(),
        r#"$_ -match ("^\s*" + [regex]::Escape($suite) + "(\s|$)")"#
    );
    assert!(script.contains(&format!("$suite = \"{SUITE_CRATE}\"")));
}

/// Both lookup branches only choose `$installer`, and one shared line then
/// forwards the arguments and status. The PATH-branch tests therefore cover
/// forwarding for the adjacent branch too, which
/// `windows_wrapper_adjacent` proves is selected.
#[test]
fn windows_wrapper_branches_share_one_forwarding_line() {
    let script = windows_whitaker_script();
    let invocations: Vec<_> = script
        .lines()
        .filter(|line| line.trim_start().starts_with('&'))
        .collect();

    assert_eq!(invocations, ["& $installer run -- @args"]);
    let (lookup, rest) = script
        .split_once("& $installer run -- @args")
        .expect("the forwarding line follows the lookup");
    assert_eq!(
        rest.trim(),
        "exit $LASTEXITCODE",
        "the status is the installer's"
    );
    let assignments: Vec<_> = lookup
        .lines()
        .filter(|line| line.contains("$installer ="))
        .map(str::trim)
        .collect();
    assert_eq!(
        assignments,
        [
            "$installer = Join-Path $PSScriptRoot 'whitaker-installer.exe'",
            "$installer = $command.Source",
        ],
        "each branch only chooses the installer"
    );
}
