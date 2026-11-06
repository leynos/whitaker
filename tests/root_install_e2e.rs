//! End-to-end coverage for the root `whitaker install` success path.
//!
//! The root binary delegates installation to the installer library, and the
//! other root tests only exercise `install --dry-run`. This test drives a real
//! installation through the public CLI using the same offline seams the
//! installer's own behaviour tests use — a fixture Clippy snapshot, synthetic
//! suite staging, and isolated data, bin, and home roots — and asserts the
//! outcomes a successful root install must produce: the staged suite library,
//! the installation record that points at it, no wrapper scripts, and the
//! diagnostics that tell the user how to run the root CLI.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;
use whitaker_installer::{
    builder::{library_extension, library_prefix},
    dirs::BaseDirs,
    installation_record::installed_paths,
    test_support::TEST_STAGE_SUITE_ENV,
    test_utils::SnapshotFixture,
};

/// Read the toolchain channel pinned by `rust-toolchain.toml`.
///
/// The fixture Clippy snapshot names the same channel, so the test installs
/// into a toolchain the manifest and the snapshot agree on.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read, parsed, or does not
/// declare a channel.
fn pinned_toolchain_channel() -> Result<String, String> {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("rust-toolchain.toml");
    let contents = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{} should be readable: {error}", manifest_path.display()))?;
    let manifest: toml::Value = toml::from_str(&contents)
        .map_err(|error| format!("{} should parse as TOML: {error}", manifest_path.display()))?;
    manifest
        .get("toolchain")
        .and_then(|toolchain| toolchain.get("channel"))
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            format!(
                "{} should declare toolchain.channel",
                manifest_path.display()
            )
        })
}

/// Report whether `rustup` can run the pinned channel.
///
/// Installation verifies the toolchain before staging anything, so a host
/// without the pinned nightly cannot exercise this path. Such a host skips the
/// test rather than failing it, matching the installer's behaviour tests.
fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The staged suite library the synthetic staging step writes.
///
/// Mirrors `Stager::staged_filename`, whose naming this test would otherwise
/// only rediscover by reading the staging directory.
fn expected_library_path(target_dir: &Path, channel: &str) -> PathBuf {
    let file_name = format!(
        "{}whitaker_suite@{channel}{}",
        library_prefix(),
        library_extension()
    );
    target_dir.join(channel).join("release").join(file_name)
}

/// Create a temporary directory, naming it in the failure message.
///
/// # Errors
///
/// Returns a description of the failed directory creation.
fn temporary_directory(label: &str) -> Result<TempDir, String> {
    TempDir::new().map_err(|error| format!("temporary {label} should be created: {error}"))
}

/// A [`BaseDirs`] whose data root is a temporary directory.
struct FixtureDirs {
    data: PathBuf,
}

impl BaseDirs for FixtureDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data.clone()) }
}

/// The isolated roots and Clippy snapshot one install run writes into.
///
/// `--skip-wrapper` is deliberately not used: leaving wrapper generation in
/// the run is what lets the test observe that a root install writes no
/// compatibility scripts, while `XDG_BIN_HOME` and `HOME` keep the check away
/// from the real user directories.
struct InstallFixture {
    channel: String,
    snapshot: SnapshotFixture,
    target_dir: TempDir,
    data_home: TempDir,
    bin_home: TempDir,
    home: TempDir,
}

impl InstallFixture {
    /// Build the isolated roots and the Clippy snapshot they install.
    ///
    /// # Errors
    ///
    /// Returns a description of the first fixture step that failed.
    fn new() -> Result<Self, String> {
        Ok(Self {
            channel: pinned_toolchain_channel()?,
            snapshot: SnapshotFixture::new()
                .map_err(|error| format!("fixture Clippy snapshot should build: {error}"))?,
            target_dir: temporary_directory("install target")?,
            data_home: temporary_directory("data home")?,
            bin_home: temporary_directory("bin home")?,
            home: temporary_directory("home")?,
        })
    }

    /// The pinned toolchain channel the fixture installs for.
    fn channel(&self) -> &str { &self.channel }

    /// The directory the synthetic staging step writes the suite into.
    fn staging_directory(&self) -> PathBuf {
        self.target_dir.path().join(&self.channel).join("release")
    }

    /// The data directories the root binary resolves through `XDG_DATA_HOME`.
    fn dirs(&self) -> FixtureDirs {
        FixtureDirs {
            data: self.data_home.path().join("whitaker"),
        }
    }

    /// Build the `whitaker install` command for an offline fixture install.
    ///
    /// `--build-only` keeps the prebuilt download out of the run and
    /// `--skip-deps` keeps the host's Dylint tools out of it, so the only work
    /// left is the staged suite, the record, and the root finalisation.
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker"));
        command
            .arg("install")
            .arg("--build-only")
            .arg("--skip-deps")
            .arg("--toolchain")
            .arg(&self.channel)
            .arg("--target-dir")
            .arg(self.target_dir.path())
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env(TEST_STAGE_SUITE_ENV, "1")
            .env("XDG_DATA_HOME", self.data_home.path())
            .env("XDG_BIN_HOME", self.bin_home.path())
            .env("HOME", self.home.path())
            .env_remove("DYLINT_LIBRARY_PATH");
        self.snapshot.configure(&mut command);
        command
    }

    /// Run the fixture install to completion.
    ///
    /// # Errors
    ///
    /// Returns a description of the failed process start.
    fn run(&self) -> Result<Output, String> {
        self.command()
            .output()
            .map_err(|error| format!("the root install process should start: {error}"))
    }
}

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
/// Returns a description of the missing staging artifact.
fn assert_staged_library(fixture: &InstallFixture) -> Result<(), String> {
    let staged = expected_library_path(fixture.target_dir.path(), fixture.channel());
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
    if recorded
        .iter()
        .any(|path| path.as_std_path() == staging.as_path())
    {
        return Ok(());
    }
    Err(format!(
        "the installation record should name {}, got {recorded:?}",
        staging.display()
    ))
}

/// Fail unless the root install wrote no compatibility wrapper scripts.
///
/// # Errors
///
/// Returns a description of the wrapper names that were written.
fn assert_no_wrapper_scripts(fixture: &InstallFixture) -> Result<(), String> {
    let names = directory_entry_names(fixture.bin_home.path())?;
    let wrappers: Vec<&String> = names
        .iter()
        .filter(|name| name.starts_with("whitaker"))
        .collect();
    if wrappers.is_empty() {
        return Ok(());
    }
    Err(format!(
        "a root install should write no wrapper scripts, found {wrappers:?}"
    ))
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
    if !is_toolchain_installed(fixture.channel()) {
        let channel = fixture.channel();
        tracing::info!(%channel, "skipping: the pinned rustup toolchain is not installed");
        return;
    }

    let output = fixture.run().expect("the root install process should run");

    assert_install_succeeded(&output).expect("install should succeed");
    assert_staged_library(&fixture).expect("the suite library should be staged");
    assert_recorded_staging_directory(&fixture)
        .expect("the installation record should name the staging directory");
    assert_no_wrapper_scripts(&fixture).expect("a root install should write no wrapper scripts");
    assert_output_contract(&output).expect("install output should describe the root CLI");
}
