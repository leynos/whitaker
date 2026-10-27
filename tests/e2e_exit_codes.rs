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

#[cfg(windows)]
#[test]
fn forwarded_windows_child_exit_codes_above_255_are_preserved() {
    use std::{env, io};

    use camino::Utf8Path;
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use tempfile::TempDir;

    let bin_dir = TempDir::new().expect("fake cargo directory should be created");
    let bin_path = Utf8Path::from_path(bin_dir.path()).expect("temporary path should be UTF-8");
    let bin_handle = Dir::open_ambient_dir(bin_path, ambient_authority())
        .expect("fake cargo directory should be opened");

    let test_executable = env::current_exe().expect("test executable path should be available");
    let executable_name = test_executable
        .file_name()
        .and_then(|name| name.to_str())
        .expect("test executable name should be UTF-8");
    let executable_directory = test_executable
        .parent()
        .and_then(Utf8Path::from_path)
        .expect("test executable directory should be UTF-8");
    let executable_directory = Dir::open_ambient_dir(executable_directory, ambient_authority())
        .expect("test executable directory should be opened");
    let mut source = executable_directory
        .open(executable_name)
        .expect("test executable should be readable");
    let mut fake_cargo = bin_handle
        .create("cargo.exe")
        .expect("fake cargo executable should be created");
    io::copy(&mut source, &mut fake_cargo).expect("test executable should be copied");
    drop(fake_cargo);

    let existing_path = env::var_os("PATH").unwrap_or_default();
    let mut search_paths = vec![bin_dir.path().as_os_str().to_owned()];
    search_paths.extend(env::split_paths(&existing_path).map(|path| path.into_os_string()));
    let search_path =
        env::join_paths(search_paths).expect("fake cargo search path should be valid");
    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .arg("--ignored")
        .env("PATH", search_path)
        .env("WHITAKER_E2E_FAKE_CARGO", "1")
        .output()
        .expect("whitaker forwarding process should start");

    assert_eq!(
        output.status.code(),
        Some(256),
        "the forwarded Windows child status should retain all 32 bits"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "subprocess fixture for Windows forwarded exit-code coverage"]
fn fake_cargo_dylint() {
    if std::env::var_os("WHITAKER_E2E_FAKE_CARGO").is_some() {
        std::process::exit(256);
    }
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

    // Bare `whitaker` must behave like the legacy wrapper's
    // `exec cargo dylint "$@"`, not print Whitaker usage.
    let bare_output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .env("PATH", bin_dir.path())
        .output()
        .expect("whitaker forwarding process should start");
    assert_eq!(
        bare_output.status.code(),
        Some(23),
        "bare whitaker should forward to cargo dylint"
    );
    assert_eq!(
        String::from_utf8_lossy(&bare_output.stdout),
        "dylint\n",
        "bare whitaker should pass no arguments beyond the dylint subcommand"
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

    bin_dir_handle
        .write("cargo", "#!/bin/sh\nkill -INT $$\n")
        .expect("interrupting cargo command should be written");
    let interrupted = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(["--all"])
        .env("PATH", bin_dir.path())
        .output()
        .expect("whitaker forwarding process should start");
    assert_eq!(
        interrupted.status.code(),
        Some(130),
        "a signal-terminated child should preserve the shell status"
    );
}
