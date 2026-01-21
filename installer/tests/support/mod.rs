//! Test support utilities for installer behavioural tests.
//!
//! This module provides common helper functions used across multiple test files,
//! including workspace path resolution, toolchain detection, and isolated rustup
//! environment setup.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use whitaker_installer::toolchain::parse_toolchain_channel;

/// Returns the workspace root directory (parent of the installer crate).
pub fn workspace_root() -> PathBuf {
    PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest dir should have parent")
        .to_owned()
}

/// Parses and returns the toolchain channel from rust-toolchain.toml.
pub fn pinned_toolchain_channel() -> String {
    let toolchain_path = workspace_root().join("rust-toolchain.toml");
    let contents =
        std::fs::read_to_string(&toolchain_path).expect("failed to read rust-toolchain.toml");
    parse_toolchain_channel(&contents).expect("failed to parse rust-toolchain.toml")
}

/// Checks if a toolchain is installed on the host system.
pub fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Checks if a toolchain is installed in an isolated rustup environment.
pub fn is_toolchain_installed_in_env(
    channel: &str,
    rustup_home: &TempDir,
    cargo_home: &TempDir,
) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .env("RUSTUP_HOME", rustup_home.path())
        .env("CARGO_HOME", cargo_home.path())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Result of setting up an isolated rustup environment.
pub struct IsolatedRustupEnv {
    pub rustup_home: TempDir,
    pub cargo_home: TempDir,
}

/// Initialises an isolated rustup environment by running `rustup show`.
///
/// This creates the necessary settings files that rustup expects to exist.
/// The function sets RUSTUP_AUTO_INSTALL=0 to prevent auto-installing any
/// toolchain during initialisation, clears RUSTUP_TOOLCHAIN to avoid
/// rust-toolchain.toml files affecting the initialisation, and runs from
/// rustup_home as the current directory to prevent rustup from walking up
/// to the workspace and discovering the project's rust-toolchain.toml (which
/// would affect toolchain selection).
fn init_isolated_rustup(rustup_home: &Path, cargo_home: &Path) {
    let init_output = Command::new("rustup")
        .arg("show")
        .current_dir(rustup_home) // Prevent rustup from discovering workspace rust-toolchain.toml
        .env("RUSTUP_HOME", rustup_home)
        .env("CARGO_HOME", cargo_home)
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .expect("failed to initialise isolated rustup environment");

    assert!(
        init_output.status.success(),
        "failed to initialise isolated rustup: {}",
        String::from_utf8_lossy(&init_output.stderr)
    );
}

/// Parses the output of a command that locates rustup, extracting the first path.
fn parse_rustup_location_output(output: &std::process::Output, _command_name: &str) -> String {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .expect("rustup not found in PATH")
        .trim()
        .to_string()
}

/// Locates the system rustup binary path.
#[cfg(unix)]
fn find_system_rustup() -> String {
    let output = Command::new("which")
        .arg("rustup")
        .output()
        .expect("failed to run which rustup");
    parse_rustup_location_output(&output, "which")
}

#[cfg(windows)]
fn find_system_rustup() -> String {
    let output = Command::new("where")
        .arg("rustup")
        .output()
        .expect("failed to run where rustup");
    parse_rustup_location_output(&output, "where")
}

/// Installs rustup into the isolated cargo_bin directory.
#[cfg(unix)]
fn install_rustup_to_cargo_bin(rustup_path: &str, cargo_bin: &Path) {
    std::os::unix::fs::symlink(rustup_path, cargo_bin.join("rustup"))
        .expect("failed to symlink rustup to CARGO_HOME/bin");
}

#[cfg(windows)]
fn install_rustup_to_cargo_bin(rustup_path: &str, cargo_bin: &Path) {
    std::fs::copy(rustup_path, cargo_bin.join("rustup.exe"))
        .expect("failed to copy rustup to CARGO_HOME/bin");
}

/// Sets up isolated RUSTUP_HOME and CARGO_HOME directories for testing.
///
/// This ensures the auto-install code path is exercised regardless of host state.
/// The function initialises rustup in the isolated environment and makes the system
/// rustup binary available (via symlink on Unix, copy on Windows).
///
/// # Panics
///
/// Panics if the isolated environment cannot be created or initialised.
pub fn setup_isolated_rustup() -> IsolatedRustupEnv {
    let rustup_home = TempDir::new().expect("failed to create RUSTUP_HOME temp dir");
    let cargo_home = TempDir::new().expect("failed to create CARGO_HOME temp dir");

    init_isolated_rustup(rustup_home.path(), cargo_home.path());

    let cargo_bin = cargo_home.path().join("bin");
    std::fs::create_dir_all(&cargo_bin).expect("failed to create CARGO_HOME/bin");

    let rustup_path = find_system_rustup();
    install_rustup_to_cargo_bin(&rustup_path, &cargo_bin);

    IsolatedRustupEnv {
        rustup_home,
        cargo_home,
    }
}
