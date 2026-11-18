//! End-to-end assertions for `whitaker check`'s process exit contract.
//!
//! These run the real binary without an installation, which is the state a new
//! user's machine is in. They pin the two things that must not drift: a
//! refused check exits with the provisioning code and names the reason on
//! stderr, and `check` is never forwarded to `cargo dylint` as a lint
//! argument.

#[cfg(unix)]
use std::process::Stdio;
use std::process::{Command, Output};

#[cfg(unix)]
use tempfile::TempDir;

// The shared fixture is written for the root-CLI behaviour suites; this one
// uses its installed bootstrap and stub `cargo`, not the record helpers.
#[cfg(unix)]
#[expect(
    dead_code,
    reason = "the shared fixture also serves the root-CLI behaviour suites"
)]
#[path = "behaviour_whitaker/support.rs"]
mod support;

/// Run the root binary with the supplied arguments and capture its output.
fn run_whitaker(arguments: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(arguments)
        .output()
}

/// Run the root binary against an empty data root, so no installation exists.
///
/// `PATH` is replaced rather than extended so an ambient `cargo dylint` can
/// never answer in place of the absent installation.
#[cfg(unix)]
fn run_without_an_installation(
    bin_dir: &TempDir,
    data_home: &TempDir,
    arguments: &[&str],
) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(arguments)
        .env("PATH", bin_dir.path())
        .env("XDG_DATA_HOME", data_home.path())
        .env_remove("DYLINT_LIBRARY_PATH")
        .output()
}

/// Create a directory holding an executable `cargo` stub that records a run.
///
/// # Errors
///
/// Returns an error when the temporary directory cannot be created or written.
#[cfg(unix)]
fn fake_cargo_dir(script: &str) -> std::io::Result<TempDir> {
    use std::os::unix::fs::PermissionsExt;

    let directory = TempDir::new()?;
    let path = directory.path().join("cargo");
    std::fs::write(&path, script)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(directory)
}

#[test]
fn check_appears_in_the_root_help() {
    let output = run_whitaker(&["--help"]).expect("whitaker help process should start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "help should exit successfully"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("check"),
        "the root help should list the check subcommand: {stdout}"
    );
}

#[test]
fn check_help_exits_successfully() {
    let output = run_whitaker(&["check", "--help"]).expect("whitaker check help should start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "check --help should exit successfully"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("check"),
        "check help should describe the command: {stdout}"
    );
}

/// `check` is a subcommand, so it must never reach the legacy forwarding path
/// that hands the whole argument vector to `cargo dylint`.
#[cfg(unix)]
#[test]
fn check_is_not_forwarded_to_cargo_dylint() {
    let record = TempDir::new().expect("a record directory should be created");
    let log = record.path().join("forwarded.log");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 23\n",
        log.display()
    );
    let bin_dir = fake_cargo_dir(&script).expect("fake cargo should be created");
    let data_home = TempDir::new().expect("an empty data directory should be created");

    let output = run_without_an_installation(&bin_dir, &data_home, &["check", "--no-install"])
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(4),
        "a refused check must not return the stub's status"
    );
    assert!(
        !log.exists(),
        "check must not forward its arguments to cargo dylint"
    );
}

/// `-p` belongs to `cargo check`, so it must follow a `--`; without one it is
/// a Whitaker usage error rather than a silently forwarded argument.
#[test]
fn cargo_options_without_a_separator_are_a_usage_error() {
    let output = run_whitaker(&["check", "-p", "x"]).expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(2),
        "a cargo option before -- is a usage error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("-p"),
        "the usage error should name the rejected option: {stderr}"
    );
}

/// Run a provisioned fixture's binary, overriding its environment per case.
///
/// # Errors
///
/// Returns an error when the fixture cannot build its command or the process
/// cannot be started.
#[cfg(unix)]
fn run_provisioned(
    fixture: &support::BootstrapFixture,
    arguments: &[&str],
    environment: &[(&str, &str)],
) -> std::io::Result<Output> {
    let mut command = fixture.command().map_err(std::io::Error::other)?;
    command.args(arguments);
    for (key, value) in environment {
        command.env(key, value);
    }
    command.output()
}

/// Dylint's own status is the answer to "did my code pass", so `main` must
/// return it untouched rather than folding it into a Whitaker code.
#[cfg(unix)]
#[test]
fn dylint_statuses_pass_through_unchanged() {
    let fixture =
        support::BootstrapFixture::new().expect("the fixture installation should be created");

    for status in ["0", "1", "23", "101"] {
        let output = run_provisioned(&fixture, &["check"], &[("STUB_EXIT", status)])
            .expect("whitaker check process should start");
        let expected = status
            .parse::<i32>()
            .expect("the case status should be a number");

        assert_eq!(
            output.status.code(),
            Some(expected),
            "Dylint's status {status} must reach the caller, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// A child killed by a signal has no exit code, so the Unix convention of
/// `128 + n` is the only status a caller can observe.
#[cfg(unix)]
#[test]
fn a_signalled_dylint_run_passes_through_128_plus_n() {
    let fixture =
        support::BootstrapFixture::new().expect("the fixture installation should be created");

    let output = run_provisioned(&fixture, &["check"], &[("STUB_SIGNAL", "TERM")])
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(143),
        "SIGTERM must be reported as 128 + 15, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `--no-install` turns a missing installation into a refusal, not an
/// attempt to change the user's machine.
#[cfg(unix)]
#[test]
fn no_install_refuses_an_unprovisioned_machine() {
    let bin_dir = fake_cargo_dir("#!/bin/sh\nexit 0\n").expect("fake cargo should be created");
    let data_home = TempDir::new().expect("an empty data directory should be created");

    let output = run_without_an_installation(&bin_dir, &data_home, &["check", "--no-install"])
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(4),
        "a refused repair exits with the provisioning code, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("whitaker: error[provision.refused]:"),
        "the refusal should name its stable key: {stderr}"
    );
    assert!(
        stderr.contains("whitaker install"),
        "the refusal should tell the user how to provision: {stderr}"
    );
}

/// A host with no `cargo` cannot start Dylint at all. That is not a lint
/// failure and not something provisioning can repair, so it reports a host
/// key on the execution code.
///
/// The user-managed route is used because it spawns `cargo` directly, which
/// makes the missing executable observable without a recorded installation.
#[cfg(unix)]
#[test]
fn a_missing_cargo_is_a_host_failure() {
    let library_path = TempDir::new().expect("a library path should be created");
    let suite = format!(
        "{}whitaker_suite@nightly-test{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    std::fs::write(library_path.path().join(suite), []).expect("a suite marker should be written");
    let empty_bin = TempDir::new().expect("an empty bin directory should be created");
    // An empty data root keeps the user-managed route authoritative: the real
    // home's installation, if any, must not answer for this test.
    let data_home = TempDir::new().expect("an empty data directory should be created");

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(["check"])
        .env("PATH", empty_bin.path())
        .env("XDG_DATA_HOME", data_home.path())
        .env("DYLINT_LIBRARY_PATH", library_path.path())
        .output()
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(5),
        "a missing cargo is a host failure, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("whitaker: error[exec.spawn]:"),
        "the host failure should name its stable key: {stderr}"
    );
}

/// Help that never reaches standard output has not been displayed, so the
/// command reports its own output failure instead of a success or a usage
/// error. `/dev/full` makes the write fail deterministically, where a closed
/// pipe would race the pipe buffer.
#[cfg(target_os = "linux")]
#[test]
fn a_lost_help_write_is_the_check_output_failure() {
    let sink = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(["check", "--help"])
        .stdout(Stdio::from(sink))
        .output()
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(5),
        "a lost diagnostic is the exec.output failure, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("whitaker: error[exec.output]:"),
        "the output failure should name its stable key: {stderr}"
    );
}
