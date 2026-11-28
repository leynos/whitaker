//! End-to-end assertions for `whitaker check`'s process exit contract.
//!
//! These drive the real binary against a fixture installation or against
//! temporary roots that hold none, which is the state a new user's machine is
//! in. They pin the codes that must not drift: a refused check exits with the
//! provisioning code and names its reason on stderr without starting Dylint,
//! Dylint's own statuses pass through untouched, and a host fault reports its
//! own key.

#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::process::Stdio;
use std::process::{Command, Output};

#[cfg(unix)]
use camino::Utf8PathBuf;
use rstest::rstest;
#[cfg(unix)]
use tempfile::TempDir;
#[cfg(unix)]
use whitaker_installer::{
    crate_name::CrateName,
    resolution::SUITE_CRATE,
    stager::Stager,
    test_support::TEST_DATA_HOME_ENV,
};

#[cfg(unix)]
mod e2e_check_support;

/// Run the root binary with the supplied arguments and capture its output.
fn run_whitaker(arguments: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(arguments)
        .output()
}

/// Point a child at temporary roots that hold no installation.
///
/// `PATH` is replaced rather than extended so an ambient `cargo dylint` can
/// never answer in place of the absent installation. Debug test builds also
/// name the project-data directory below `data_home` through the installer's
/// test override, because a platform whose native data directory ignores
/// `XDG_DATA_HOME` would otherwise consult the developer's installation.
/// Release test builds ignore the override and resolve the native directory;
/// `HOME` and `XDG_DATA_HOME` still keep that directory inside `data_home` on
/// Unix.
#[cfg(unix)]
fn isolate_outside_an_installation(command: &mut Command, data_home: &Path) {
    command
        .env("XDG_DATA_HOME", data_home)
        .env("HOME", data_home);
    if cfg!(debug_assertions) {
        command.env(TEST_DATA_HOME_ENV, data_home.join("whitaker"));
    }
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

/// Verify that both help views advertise the `check` subcommand.
///
/// The cases distinguish advertising `check` in the root help from displaying
/// help for `check` itself; both must exit successfully and name the command.
#[rstest]
#[case::root_help(&["--help"], "the root help should list the check subcommand")]
#[case::check_help(&["check", "--help"], "check help should describe the command")]
fn help_documents_check(#[case] arguments: &[&str], #[case] expectation: &str) {
    let Ok(output) = run_whitaker(arguments) else {
        panic!("{arguments:?} should start the whitaker process");
    };

    assert_eq!(
        output.status.code(),
        Some(0),
        "{arguments:?} should exit successfully"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("check"), "{expectation}: {stdout}");
}

/// Refuse an unprovisioned machine without starting Dylint.
///
/// The recording stub proves the whole refusal contract from one launch: the
/// check exits with the provisioning code rather than the stub's status, the
/// log stays absent because Dylint is never invoked, and stderr names both the
/// stable reason and the install command that would provision the machine.
#[cfg(unix)]
#[test]
fn no_install_refuses_without_invoking_dylint() {
    let record = TempDir::new().expect("a record directory should be created");
    let log = record.path().join("forwarded.log");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 23\n",
        log.display()
    );
    let bin_dir = fake_cargo_dir(&script).expect("fake cargo should be created");
    let data_home = TempDir::new().expect("an empty data directory should be created");

    let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker"));
    command
        .args(["check", "--no-install"])
        .env("PATH", bin_dir.path())
        .env_remove("DYLINT_LIBRARY_PATH");
    isolate_outside_an_installation(&mut command, data_home.path());
    let output = command
        .output()
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(4),
        "a refused check must exit with the provisioning code, not the stub's status, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !log.exists(),
        "check must not forward its arguments to cargo dylint"
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

/// Add a second staged suite to the fixture's recorded library directory.
///
/// The added library carries a different toolchain suffix, so it parses as a
/// Whitaker suite and puts the recorded directory in the state Dylint refuses
/// to choose within. Only this suite needs the extra library, so the path is
/// derived here rather than widening the shared fixture.
///
/// # Errors
///
/// Returns an error when the second library cannot be written.
#[cfg(unix)]
fn add_second_suite(
    fixture: &e2e_check_support::BootstrapFixture,
    toolchain: &str,
) -> std::io::Result<()> {
    let filename = Stager::new(Utf8PathBuf::from("unused"), toolchain)
        .staged_filename(&CrateName::from(SUITE_CRATE));
    std::fs::write(fixture.data_root().join("lints/lib").join(filename), [])
}

/// Refuse a recorded directory naming two suites, before Dylint can exit `1`.
///
/// Dylint 6.3.0 rejects a `--lib` name that resolves to more than one library,
/// and its status for that is the status it uses for a lint denial. The
/// ambiguity must therefore be refused before the child starts, or `check`
/// would pass a provisioning defect through as a lint result. The stub's own
/// log, pointed at a path this test owns, is the evidence the child never ran.
#[cfg(unix)]
#[test]
fn a_recorded_directory_with_two_suites_is_refused_before_dylint() {
    let fixture = e2e_check_support::BootstrapFixture::new()
        .expect("the fixture installation should be created");
    add_second_suite(&fixture, "nightly-stale").expect("a second staged suite should be written");
    let record = TempDir::new().expect("a child-record directory should be created");
    let log_path = record.path().join("dylint-ran");
    let child_log = log_path
        .to_str()
        .expect("the child log path should be UTF-8");

    let output = run_provisioned(
        &fixture,
        &["check", "--no-install"],
        &[("STUB_LOG", child_log)],
    )
    .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(4),
        "an ambiguous installation is a provisioning refusal, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !Path::new(child_log).exists(),
        "check must not start Dylint with a library directory it cannot choose within"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("whitaker: error[provision.refused]:"),
        "the refusal should name its stable key: {stderr}"
    );
    assert!(
        stderr.contains("name more than one suite, so Dylint cannot choose one"),
        "the diagnosis should name the ambiguity: {stderr}"
    );
}

/// A Cargo option Whitaker does not model is still a usage error before `--`.
///
/// Package, target and feature selectors are native now, but an output option
/// such as `--message-format` has no Whitaker meaning and must be written after
/// the separator.
#[test]
fn a_cargo_only_option_without_a_separator_is_a_usage_error() {
    let output = run_whitaker(&["check", "--message-format=json"])
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(2),
        "an option Whitaker does not model is a usage error before --"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--message-format"),
        "the usage error should name the rejected option: {stderr}"
    );
}

/// A native package selector reaches Cargo unchanged, after the separator.
#[cfg(unix)]
#[test]
fn a_native_package_selector_reaches_cargo_unchanged() {
    let fixture = e2e_check_support::BootstrapFixture::new()
        .expect("the fixture installation should be created");
    let log_directory = TempDir::new().expect("a temporary directory is available");
    let log = log_directory.path().join("stub.log");
    let log_text = log.to_str().expect("the log path is UTF-8");

    let output = run_provisioned(&fixture, &["check", "-p", "x"], &[("STUB_LOG", log_text)])
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(23),
        "the child's status must pass through, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let recorded = std::fs::read_to_string(&log).expect("the stub log should be readable");
    let arguments: Vec<&str> = recorded
        .lines()
        .filter_map(|line| line.strip_prefix("ARG="))
        .collect();
    let separator = arguments
        .iter()
        .position(|argument| *argument == "--")
        .expect("the assembled vector should hold a separator");
    assert!(
        arguments
            .get(separator..)
            .is_some_and(|tail| tail.windows(2).any(|pair| pair == ["-p", "x"])),
        "`-p x` must reach Cargo after Whitaker's separator, got {arguments:?}"
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
    fixture: &e2e_check_support::BootstrapFixture,
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
    let fixture = e2e_check_support::BootstrapFixture::new()
        .expect("the fixture installation should be created");

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
    let fixture = e2e_check_support::BootstrapFixture::new()
        .expect("the fixture installation should be created");

    let output = run_provisioned(&fixture, &["check"], &[("STUB_SIGNAL", "TERM")])
        .expect("whitaker check process should start");

    assert_eq!(
        output.status.code(),
        Some(143),
        "SIGTERM must be reported as 128 + 15, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
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
    // Empty roots keep the user-managed route authoritative: the real home's
    // installation, if any, must not answer for this test.
    let data_home = TempDir::new().expect("an empty data directory should be created");

    let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker"));
    command
        .args(["check"])
        .env("PATH", empty_bin.path())
        .env("DYLINT_LIBRARY_PATH", library_path.path());
    isolate_outside_an_installation(&mut command, data_home.path());
    let output = command
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
