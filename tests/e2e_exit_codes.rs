//! End-to-end assertions for the root binary's process exit contract.

use std::process::Command;

#[test]
fn root_help_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .arg("--help")
        .output()
        .expect("whitaker help process should start");
    assert_eq!(
        output.status.code(),
        Some(0),
        "help should exit successfully"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("install"), "help should list install");
    assert!(stdout.contains("ls"), "help should list ls");
}

#[test]
fn root_help_subcommand_is_not_forwarded_to_dylint() {
    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .arg("help")
        .output()
        .expect("whitaker help process should start");
    assert_eq!(
        output.status.code(),
        Some(0),
        "the help subcommand should exit successfully"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: whitaker"),
        "unexpected help: {stdout}"
    );
    assert!(output.stderr.is_empty(), "help should not report an error");
}

#[test]
fn install_usage_errors_use_clap_exit_code_two() {
    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args([
            "install",
            "--lint",
            "module_max_lines",
            "--individual-lints",
        ])
        .output()
        .expect("whitaker install process should start");
    assert_eq!(
        output.status.code(),
        Some(2),
        "usage errors should exit with code 2"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--lint"), "error should name --lint");
    assert!(
        stderr.contains("--individual-lints"),
        "error should name --individual-lints"
    );
}

#[cfg(unix)]
#[test]
fn legacy_dylint_invocations_forward_arguments_and_exit_status() {
    use cap_std::{
        ambient_authority,
        fs_utf8::{Dir, Permissions, PermissionsExt},
    };
    use tempfile::TempDir;

    let bin_dir = TempDir::new().expect("fake cargo directory should be created");
    let bin_dir_handle = Dir::open_ambient_dir(
        camino::Utf8Path::from_path(bin_dir.path()).expect("temporary path should be UTF-8"),
        ambient_authority(),
    )
    .expect("fake cargo directory should be opened");
    bin_dir_handle
        .write("cargo", "#!/bin/sh\nprintf '%s\\n' \"$@\"\nexit 23\n")
        .expect("fake cargo command should be written");
    bin_dir_handle
        .set_permissions("cargo", Permissions::from_mode(0o755))
        .expect("fake cargo should be executable");

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(["--all", "--", "-p", "whitaker-common", "--all-targets"])
        .env("PATH", bin_dir.path())
        .output()
        .expect("whitaker forwarding process should start");
    assert_eq!(
        output.status.code(),
        Some(23),
        "forwarded status should be preserved"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("dylint\n--all\n--\n"),
        "cargo dylint args should be preserved"
    );
    assert!(
        stdout.contains("whitaker-common"),
        "cargo arguments should reach the fake process"
    );

    let version_output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(["--all", "--version"])
        .env("PATH", bin_dir.path())
        .output()
        .expect("whitaker forwarding process should start");
    assert_eq!(
        version_output.status.code(),
        Some(23),
        "Dylint's --version option should be forwarded after --all",
    );
    let version_stdout = String::from_utf8_lossy(&version_output.stdout);
    assert!(
        version_stdout.starts_with("dylint\n--all\n--version\n"),
        "cargo dylint should receive the later version option: {version_stdout:?}",
    );
}
