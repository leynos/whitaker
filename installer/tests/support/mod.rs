//! Test support utilities for installer behavioural tests.
//!
//! This module provides common helper functions used across multiple test files,
//! including workspace path resolution, toolchain detection, and isolated rustup
//! environment setup.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::TempDir;
use whitaker_installer::toolchain::parse_toolchain_channel;

/// Returns the workspace root directory (parent of the installer crate).
///
/// # Errors
///
/// Returns an error when the installer manifest directory has no parent.
pub fn workspace_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| String::from("installer crate is not at workspace root"))
}

/// Parses and returns the toolchain channel from rust-toolchain.toml.
///
/// # Errors
///
/// Returns an error when rust-toolchain.toml cannot be read or parsed.
pub fn pinned_toolchain_channel() -> Result<String, String> {
    let toolchain_path = workspace_root()?.join("rust-toolchain.toml");
    let contents = std::fs::read_to_string(&toolchain_path)
        .map_err(|error| format!("failed to read {}: {error}", toolchain_path.display()))?;
    parse_toolchain_channel(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", toolchain_path.display()))
}

/// Checks if a toolchain is installed on the host system.
///
/// Sanitizes rustup environment by always setting `RUSTUP_AUTO_INSTALL=0` and
/// `RUSTUP_TOOLCHAIN` to prevent host settings from leaking into tests.
pub fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Checks if a toolchain is installed in an isolated rustup environment.
///
/// Uses the same environment sanitization as `is_toolchain_installed` to prevent
/// host settings from affecting test behaviour.
pub fn is_toolchain_installed_in_env(
    channel: &str,
    rustup_home: &TempDir,
    cargo_home: &TempDir,
) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .env("RUSTUP_HOME", rustup_home.path())
        .env("CARGO_HOME", cargo_home.path())
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Result of setting up an isolated rustup environment.
pub struct IsolatedRustupEnv {
    pub rustup_home: TempDir,
    pub cargo_home: TempDir,
}

/// Initializes an isolated rustup environment by running `rustup show`.
///
/// This creates necessary settings files that rustup expects to exist.
/// The function sets `RUSTUP_AUTO_INSTALL=0` to prevent auto-installing any
/// toolchain during initialization, clears `RUSTUP_TOOLCHAIN` to avoid
/// rust-toolchain.toml files affecting initialization, and runs from
/// `rustup_home` as a current directory to prevent rustup from walking up
/// to the workspace and discovering a project's rust-toolchain.toml
/// (which would affect toolchain selection).
///
/// # Errors
///
/// Returns an error when rustup cannot be run or reports a failure.
fn init_isolated_rustup(rustup_home: &Path, cargo_home: &Path) -> Result<(), String> {
    let init_output = Command::new("rustup")
        .arg("show")
        .current_dir(rustup_home) // Prevent rustup from discovering workspace rust-toolchain.toml
        .env("RUSTUP_HOME", rustup_home)
        .env("CARGO_HOME", cargo_home)
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .map_err(|error| format!("failed to initialize isolated rustup environment: {error}"))?;

    if !init_output.status.success() {
        return Err(format!(
            "failed to initialize isolated rustup: {}",
            String::from_utf8_lossy(&init_output.stderr)
        ));
    }

    let self_update_output = Command::new("rustup")
        .args(["set", "auto-self-update", "disable"])
        .current_dir(rustup_home)
        .env("RUSTUP_HOME", rustup_home)
        .env("CARGO_HOME", cargo_home)
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .map_err(|error| {
            format!("failed to disable rustup self-update in isolated environment: {error}")
        })?;

    if !self_update_output.status.success() {
        return Err(format!(
            "failed to disable rustup self-update: {}",
            String::from_utf8_lossy(&self_update_output.stderr)
        ));
    }

    Ok(())
}

/// Parses the output of a command that locates rustup, extracting the first path.
///
/// # Errors
///
/// Returns an error when the command produced no output lines.
fn parse_rustup_location_output(output: &std::process::Output) -> Result<String, String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_owned())
        .ok_or_else(|| String::from("rustup not found in PATH"))
}

/// Locates the system rustup binary path.
///
/// # Errors
///
/// Returns an error when the lookup command cannot run or rustup is absent.
#[cfg(unix)]
fn find_system_rustup() -> Result<String, String> {
    let output = Command::new("which")
        .arg("rustup")
        .output()
        .map_err(|error| format!("failed to run which rustup: {error}"))?;
    parse_rustup_location_output(&output)
}

#[cfg(windows)]
fn find_system_rustup() -> Result<String, String> {
    let output = Command::new("where")
        .arg("rustup")
        .output()
        .map_err(|error| format!("failed to run where rustup: {error}"))?;
    parse_rustup_location_output(&output)
}

/// Installs rustup into the isolated `cargo_bin` directory.
///
/// # Errors
///
/// Returns an error when rustup cannot be linked or copied into `cargo_bin`.
#[cfg(unix)]
fn install_rustup_to_cargo_bin(rustup_path: &str, cargo_bin: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(rustup_path, cargo_bin.join("rustup"))
        .map_err(|error| format!("failed to symlink rustup to CARGO_HOME/bin: {error}"))
}

#[cfg(windows)]
fn install_rustup_to_cargo_bin(rustup_path: &str, cargo_bin: &Path) -> Result<(), String> {
    std::fs::copy(rustup_path, cargo_bin.join("rustup.exe"))
        .map(|_| ())
        .map_err(|error| format!("failed to copy rustup to CARGO_HOME/bin: {error}"))
}

/// Sets up isolated `RUSTUP_HOME` and `CARGO_HOME` directories for testing.
///
/// This ensures the auto-install code path is exercised regardless of host state.
/// The function initializes rustup in the isolated environment and makes the system
/// rustup binary available (via symlink on Unix, copy on Windows).
///
/// # Errors
///
/// Returns an error if the isolated environment cannot be created or
/// initialized.
pub fn setup_isolated_rustup() -> Result<IsolatedRustupEnv, String> {
    let rustup_home = TempDir::new()
        .map_err(|error| format!("failed to create RUSTUP_HOME temp dir: {error}"))?;
    let cargo_home =
        TempDir::new().map_err(|error| format!("failed to create CARGO_HOME temp dir: {error}"))?;

    init_isolated_rustup(rustup_home.path(), cargo_home.path())?;

    let cargo_bin = cargo_home.path().join("bin");
    std::fs::create_dir_all(&cargo_bin)
        .map_err(|error| format!("failed to create CARGO_HOME/bin: {error}"))?;

    let rustup_path = find_system_rustup()?;
    install_rustup_to_cargo_bin(&rustup_path, &cargo_bin)?;

    Ok(IsolatedRustupEnv {
        rustup_home,
        cargo_home,
    })
}
