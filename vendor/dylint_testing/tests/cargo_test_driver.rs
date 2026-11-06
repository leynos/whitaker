//! Exercise the driver at the boundary a caller actually observes.
//!
//! The unit tests check how commands are constructed. These tests run the
//! entry point the example calls: they supply the prepared metadata, route to
//! real child processes, and check the status and bytes the driver reports.

use dylint_testing::cargo_test_driver::{DriverEnvironment, run};
use std::{
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

/// The Cargo that invoked this test, falling back to a `PATH` lookup.
fn cargo() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

/// A path that no platform is expected to resolve.
fn missing_program(name: &str) -> PathBuf {
    Path::new("/nonexistent").join(name)
}

/// A metadata request naming the prepared workspace is answered from the file.
#[test]
fn serves_the_prepared_metadata_file() -> io::Result<()> {
    let workspace = tempfile::tempdir()?;
    let cache = workspace.path().join("metadata.json");
    fs::write(&cache, br#"{"packages":[]}"#)?;
    let environment = DriverEnvironment {
        metadata_root: Some(workspace.path().to_path_buf()),
        metadata_cache: Some(cache),
        cargo_executable: Some(missing_program("cargo").into_os_string()),
        nextest_executable: None,
    };
    let mut stdout = Vec::new();

    let code = run(
        &[
            "metadata".into(),
            "--no-deps".into(),
            "--format-version".into(),
            "1".into(),
        ],
        workspace.path(),
        &environment,
        &mut stdout,
    )?;

    assert_eq!(code, 0, "serving cached metadata should succeed");
    assert_eq!(
        stdout, br#"{"packages":[]}"#,
        "the exact prepared bytes should be served"
    );
    assert!(
        environment
            .cargo_executable
            .as_deref()
            .is_some_and(|cargo| !Path::new(cargo).exists()),
        "the missing Cargo proves no child was needed"
    );
    Ok(())
}

/// Metadata for a different directory is forwarded instead of cached.
#[test]
fn refuses_to_serve_metadata_for_another_directory() -> io::Result<()> {
    let workspace = tempfile::tempdir()?;
    let elsewhere = tempfile::tempdir()?;
    let cache = workspace.path().join("metadata.json");
    fs::write(&cache, b"prepared")?;
    let environment = DriverEnvironment {
        metadata_root: Some(workspace.path().to_path_buf()),
        metadata_cache: Some(cache),
        cargo_executable: Some(missing_program("cargo").into_os_string()),
        nextest_executable: None,
    };
    let mut stdout = Vec::new();

    let error = run(
        &["metadata".into(), "--no-deps".into()],
        elsewhere.path(),
        &environment,
        &mut stdout,
    )
    .expect_err("a foreign directory should be forwarded to Cargo");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(stdout.is_empty(), "the cache must not be served elsewhere");
    Ok(())
}

/// A manifest request for a file that is gone is forwarded, not cached.
///
/// The workspace directory still resolves, so a lexical comparison would call
/// the request a match and hand back metadata for a manifest that no longer
/// exists. The caller needs Cargo's own "manifest not found" error instead.
#[test]
fn forwards_a_manifest_request_whose_file_is_missing() -> io::Result<()> {
    let workspace = tempfile::tempdir()?;
    let cache = workspace.path().join("metadata.json");
    fs::write(&cache, b"prepared")?;
    let environment = DriverEnvironment {
        metadata_root: Some(workspace.path().to_path_buf()),
        metadata_cache: Some(cache),
        cargo_executable: Some(missing_program("cargo").into_os_string()),
        nextest_executable: None,
    };
    let mut stdout = Vec::new();

    let error = run(
        &[
            "metadata".into(),
            "--no-deps".into(),
            "--manifest-path".into(),
            "Cargo.toml".into(),
        ],
        workspace.path(),
        &environment,
        &mut stdout,
    )
    .expect_err("a missing manifest should be forwarded to Cargo");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        stdout.is_empty(),
        "the cache must not answer for a lost file"
    );
    Ok(())
}

/// A full metadata query is forwarded even for the prepared workspace.
#[test]
fn forwards_a_full_metadata_query() -> io::Result<()> {
    let workspace = tempfile::tempdir()?;
    let cache = workspace.path().join("metadata.json");
    fs::write(&cache, b"prepared")?;
    let environment = DriverEnvironment {
        metadata_root: Some(workspace.path().to_path_buf()),
        metadata_cache: Some(cache),
        cargo_executable: Some(missing_program("cargo").into_os_string()),
        nextest_executable: None,
    };
    let mut stdout = Vec::new();

    let error = run(
        &["metadata".into(), "--format-version".into(), "1".into()],
        workspace.path(),
        &environment,
        &mut stdout,
    )
    .expect_err("a full metadata query should be forwarded to Cargo");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(stdout.is_empty(), "the cache must not answer a full query");
    Ok(())
}

/// The exit status of the forwarded child becomes the driver's status.
#[test]
fn propagates_the_child_exit_code() -> io::Result<()> {
    let environment = DriverEnvironment {
        cargo_executable: Some(cargo()),
        ..DriverEnvironment::default()
    };
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));

    let success = run(
        &["--version".into()],
        workspace,
        &environment,
        &mut Vec::new(),
    )?;
    let failure = run(
        &["definitely-not-a-cargo-subcommand".into()],
        workspace,
        &environment,
        &mut Vec::new(),
    )?;

    assert_eq!(success, 0, "Cargo's own success should be reported");
    assert_eq!(failure, 101, "Cargo's error exit code should be reported");
    Ok(())
}

/// A forwarded call needs Cargo, and its absence is reported as such.
#[test]
fn reports_a_missing_cargo_executable() {
    let environment = DriverEnvironment::default();
    let mut stdout = Vec::new();

    let error = run(
        &["build".into()],
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &environment,
        &mut stdout,
    )
    .expect_err("a forwarded call without Cargo should fail");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        error.to_string().contains("CARGO_REAL"),
        "the error should name the variable: {error}"
    );
    assert!(stdout.is_empty(), "nothing should be written");
}

/// A matched metadata request fails when its prepared file is absent.
#[test]
fn reports_a_missing_metadata_cache() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let environment = DriverEnvironment {
        metadata_root: Some(workspace.to_path_buf()),
        metadata_cache: Some(workspace.join("target/no-such-metadata.json")),
        cargo_executable: Some(missing_program("cargo").into_os_string()),
        nextest_executable: None,
    };
    let mut stdout = Vec::new();

    let error = run(
        &["metadata".into(), "--no-deps".into()],
        workspace,
        &environment,
        &mut stdout,
    )
    .expect_err("a matched request without its cache should fail");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(stdout.is_empty(), "nothing should be written");
}

/// `nextest run` is routed to the standalone executable, not to Cargo.
#[test]
fn routes_nextest_to_the_standalone_executable() {
    let environment = DriverEnvironment {
        cargo_executable: Some(cargo()),
        nextest_executable: Some(missing_program("nextest-probe").into_os_string()),
        ..DriverEnvironment::default()
    };

    let error = run(
        &["nextest".into(), "run".into(), "--version".into()],
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &environment,
        &mut Vec::new(),
    )
    .expect_err("the missing Nextest proves Cargo was not the child");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}
