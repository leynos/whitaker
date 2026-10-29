//! The generated scripts, executed against a stub `whitaker-installer`.

use super::*;
#[cfg(unix)]
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
