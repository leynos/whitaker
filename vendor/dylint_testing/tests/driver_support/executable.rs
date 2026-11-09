//! Exercise the built driver executable as a child process.
//!
//! The library tests call `run` directly, so they cannot show that the example
//! resolves its variables from the environment, writes the cache to stdout, or
//! turns a failure into a nonzero exit. These tests start the built example and
//! observe only what a real caller sees.
//!
//! The example is located through `CARGO_TEST_DRIVER_EXAMPLE`, which `make
//! test` points at the binary `make fetch` built. A missing example fails the
//! test: a skip would let the suite pass without proving any of the behaviours
//! above.

use std::{
    env,
    ffi::OsString,
    fs,
    io,
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::TempDir;

/// Prepared metadata the driver is expected to serve unchanged.
const PREPARED_METADATA: &[u8] = br#"{"packages":[]}"#;

/// A Cargo subcommand no installation defines, used to obtain a nonzero status.
const REJECTED_SUBCOMMAND: &str = "definitely-not-a-cargo-subcommand";

/// Names the example binary the preparation step built.
const DRIVER_EXAMPLE_VARIABLE: &str = "CARGO_TEST_DRIVER_EXAMPLE";

/// The Cargo that invoked this test, falling back to a `PATH` lookup.
fn cargo() -> OsString { env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")) }

/// A path that no platform is expected to resolve.
fn missing_program(name: &str) -> PathBuf { Path::new("/nonexistent").join(name) }

/// Locate the built example, failing when it is not there.
///
/// `CARGO_TEST_DRIVER_EXAMPLE` is authoritative when set, because the
/// preparation step builds the example in its own target directory rather than
/// beside the test binary. Otherwise Cargo's layout puts examples in
/// `<profile>/examples`, so a directly invoked `cargo test` can still find one
/// it built itself. Both paths must resolve to a file: returning early instead
/// would report success without exercising the executable.
///
/// # Errors
///
/// Return an error if the named path is not a file, or if neither location
/// holds the example.
fn driver_example() -> io::Result<PathBuf> {
    if let Some(configured) = env::var_os(DRIVER_EXAMPLE_VARIABLE) {
        return existing_file(PathBuf::from(configured), DRIVER_EXAMPLE_VARIABLE);
    }

    let executable = env::current_exe()?;
    let profile = executable.parent().and_then(Path::parent).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} has no profile directory", executable.display()),
        )
    })?;
    let candidate = profile
        .join("examples")
        .join(format!("cargo_test_driver{}", env::consts::EXE_SUFFIX));

    existing_file(candidate, "beside the test binary")
}

/// Require `path` to name a built example, describing `source` on failure.
///
/// # Errors
///
/// Return an error when `path` is not a file.
fn existing_file(path: PathBuf, source: &str) -> io::Result<PathBuf> {
    if path.is_file() {
        return Ok(path);
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "no built cargo_test_driver example at {} ({source}); run `make fetch` first",
            path.display()
        ),
    ))
}

/// Create a workspace whose root manifest and prepared cache both exist.
fn prepared_workspace() -> io::Result<TempDir> {
    let workspace = tempfile::tempdir()?;
    fs::write(
        workspace.path().join("Cargo.toml"),
        b"[package]\nname = \"prepared\"\n",
    )?;
    fs::write(workspace.path().join("metadata.json"), PREPARED_METADATA)?;
    Ok(workspace)
}

/// Start the driver in a workspace, resolving its environment from the child.
fn driver(example: &Path, workspace: &Path) -> Command {
    let mut command = Command::new(example);
    command
        .current_dir(workspace)
        .env("CARGO_METADATA_ROOT", workspace)
        .env("CARGO_METADATA_CACHE", workspace.join("metadata.json"))
        .env("CARGO_REAL", cargo())
        .env_remove("CARGO_NEXTEST_REAL");
    command
}

/// The prepared bytes reach a caller's stdout through the executable.
#[test]
fn serves_the_prepared_cache_through_the_executable() -> io::Result<()> {
    let example = driver_example()?;
    let workspace = prepared_workspace()?;

    let output = driver(&example, workspace.path())
        .args(["metadata", "--no-deps"])
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, PREPARED_METADATA);
    Ok(())
}

/// The status the child reports becomes the executable's own exit status.
#[test]
fn forwards_the_child_exit_status_through_the_executable() -> io::Result<()> {
    let example = driver_example()?;
    let workspace = tempfile::tempdir()?;

    let direct = Command::new(cargo())
        .arg(REJECTED_SUBCOMMAND)
        .current_dir(workspace.path())
        .output()?;
    let forwarded = driver(&example, workspace.path())
        .arg(REJECTED_SUBCOMMAND)
        .output()?;

    assert!(!direct.status.success(), "the probe command must fail");
    assert_eq!(forwarded.status.code(), direct.status.code());
    Ok(())
}

/// A missing cache fails before a child exists, so the executable reports it.
#[test]
fn reports_a_missing_metadata_cache_through_the_executable() -> io::Result<()> {
    let example = driver_example()?;
    let workspace = prepared_workspace()?;

    let output = driver(&example, workspace.path())
        .args(["metadata", "--no-deps"])
        .env("CARGO_METADATA_CACHE", workspace.path().join("absent.json"))
        .output()?;

    assert_eq!(
        output.status.code(),
        Some(1),
        "an error maps to exit code 1"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("prepared metadata cache"),
        "the failure should name the operation: {stderr}"
    );
    assert!(output.stdout.is_empty(), "nothing should be written");
    Ok(())
}

/// Route `nextest run` to `CARGO_NEXTEST_REAL`, not to Cargo.
///
/// The configured Nextest executable does not exist, so a routed call fails
/// while starting its child. Cargo would instead report an unknown
/// subcommand, which distinguishes the two paths without needing Nextest
/// installed on the runner.
#[test]
fn routes_nextest_to_the_configured_executable() -> io::Result<()> {
    let example = driver_example()?;
    let workspace = tempfile::tempdir()?;

    let output = driver(&example, workspace.path())
        .args(["nextest", "run"])
        .env("CARGO_NEXTEST_REAL", missing_program("cargo-nextest"))
        .output()?;

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("starting the routed child"),
        "the routed executable should be the one that failed: {stderr}"
    );
    Ok(())
}

/// A missing `CARGO_REAL` fails before a child exists, and says so.
#[test]
fn reports_a_missing_cargo_executable_through_the_executable() -> io::Result<()> {
    let example = driver_example()?;
    let workspace = tempfile::tempdir()?;

    let output = driver(&example, workspace.path())
        .arg("build")
        .env_remove("CARGO_REAL")
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CARGO_REAL"),
        "the failure should name the variable: {stderr}"
    );
    Ok(())
}
