//! End-to-end coverage for the root `whitaker install` success path.
//!
//! The root binary delegates installation to the installer library, and the
//! other root tests only exercise `install --dry-run`. This test drives a real
//! installation through the public CLI using the same offline seams the
//! installer's own behaviour tests use — a fixture Clippy snapshot, synthetic
//! suite staging, and isolated data, bin, and home roots — and asserts the
//! outcomes a successful root install must produce: the staged suite library,
//! the installation record that points at it, an untouched pre-existing
//! `whitaker` executable in the bin directory, no compatibility scripts beside
//! it, and the diagnostics that tell the user how to run the root CLI.

mod root_install_fixture;

use std::{path::Path, process::Output};

use root_install_fixture::{
    InstallFixture,
    SEEDED_BINARY_BYTES,
    SEEDED_BINARY_NAME,
    expected_library_path,
    toolchain_is_available,
};
use whitaker_installer::installation_record::installed_paths;

/// Read the entry names directly under `path`.
///
/// # Errors
///
/// Returns a description of the failed directory read.
fn directory_entry_names(path: &Path) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(path)
        .map_err(|error| format!("{} should be listable: {error}", path.display()))?;
    entries
        .map(|entry| {
            entry
                .map(|listed| listed.file_name().to_string_lossy().into_owned())
                .map_err(|error| format!("{} should list: {error}", path.display()))
        })
        .collect()
}

/// Fail unless the install process exited successfully.
///
/// # Errors
///
/// Returns the captured output when the process failed.
fn assert_install_succeeded(output: &Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "install should succeed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    ))
}

/// Fail unless the suite library was staged where the record expects it.
///
/// # Errors
///
/// Returns a description of the missing staging artefact.
fn assert_staged_library(fixture: &InstallFixture) -> Result<(), String> {
    let staged = expected_library_path(fixture.target_root(), fixture.channel());
    if staged.is_file() {
        return Ok(());
    }
    Err(format!(
        "the staged suite library should exist at {}",
        staged.display()
    ))
}

/// Fail unless the installation record names the staging directory.
///
/// # Errors
///
/// Returns the record-read failure or a mismatch description.
fn assert_recorded_staging_directory(fixture: &InstallFixture) -> Result<(), String> {
    let recorded = installed_paths(&fixture.dirs())
        .map_err(|error| format!("the installation record should be readable: {error}"))?;
    let staging = fixture.staging_directory();
    // The install records the canonical staging path, and on Windows that form
    // is the extended-length `\\?\` path with any 8.3 short names expanded, so
    // the expected path has to be canonicalized the same way before comparing.
    let expected = staging.canonicalize().map_err(|error| {
        format!(
            "the staged directory {} should be resolvable: {error}",
            staging.display()
        )
    })?;
    if recorded.iter().any(|path| path.as_std_path() == expected) {
        return Ok(());
    }
    Err(format!(
        "the installation record should name {}, got {recorded:?}",
        expected.display()
    ))
}

/// Fail unless the root install wrote no compatibility wrapper scripts.
///
/// The fixture's own `whitaker` executable is expected, so it is excluded; any
/// other entry beginning with `whitaker` is a script the root install should
/// never create.
///
/// # Errors
///
/// Returns a description of the unexpected entry names.
fn assert_no_wrapper_scripts(fixture: &InstallFixture) -> Result<(), String> {
    let names = directory_entry_names(fixture.bin_root())?;
    let unexpected: Vec<&String> = names
        .iter()
        .filter(|name| name.starts_with("whitaker") && name.as_str() != SEEDED_BINARY_NAME)
        .collect();
    if unexpected.is_empty() {
        return Ok(());
    }
    Err(format!(
        "a root install should write no compatibility scripts, found {unexpected:?}"
    ))
}

/// Fail unless the pre-existing executable survived the install unchanged.
///
/// # Errors
///
/// Returns a description of the missing or modified file.
fn assert_existing_binary_preserved(fixture: &InstallFixture) -> Result<(), String> {
    let path = fixture.seeded_binary();
    let contents = std::fs::read(&path).map_err(|error| {
        format!(
            "the pre-existing {} should survive: {error}",
            path.display()
        )
    })?;
    if contents != SEEDED_BINARY_BYTES {
        return Err(format!(
            "the root install rewrote {}, got {:?}",
            path.display(),
            String::from_utf8_lossy(&contents)
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path)
            .map_err(|error| format!("{} should be readable: {error}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o111 == 0 {
            return Err(format!(
                "{} should stay executable, mode {mode:o}",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Fail unless the install output describes the root CLI contract.
///
/// # Errors
///
/// Returns a description of the missing marker or guidance line.
fn assert_output_contract(output: &Output) -> Result<(), String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("suite-source=source") {
        return Err(format!(
            "standard output should carry the source suite marker, got:\n{stdout}"
        ));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("whitaker ls") {
        return Err(format!(
            "standard error should report the root CLI commands, got:\n{stderr}"
        ));
    }
    Ok(())
}

#[test]
fn root_install_stages_the_suite_and_records_its_directory() {
    let fixture = InstallFixture::new().expect("the install fixture should be built");
    if !toolchain_is_available(&fixture) {
        return;
    }

    let output = fixture.run().expect("the root install process should run");

    assert_install_succeeded(&output).expect("install should succeed");
    assert_staged_library(&fixture).expect("the suite library should be staged");
    assert_recorded_staging_directory(&fixture)
        .expect("the installation record should name the staging directory");
    assert_no_wrapper_scripts(&fixture).expect("a root install should write no wrapper scripts");
    assert_existing_binary_preserved(&fixture)
        .expect("a root install should preserve an existing whitaker executable");
    assert_output_contract(&output).expect("install output should describe the root CLI");
}
