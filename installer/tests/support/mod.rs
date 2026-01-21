//! Test support utilities for installer behavioural tests.
//!
//! This module provides common helper functions used across multiple test files,
//! including workspace path resolution, toolchain detection, and isolated rustup
//! environment setup.

use std::path::PathBuf;
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

/// Sets up isolated RUSTUP_HOME and CARGO_HOME directories for testing.
///
/// This ensures the auto-install code path is exercised regardless of host state.
/// The function initialises rustup in the isolated environment and creates the
/// necessary symlinks (Unix) or copies (Windows) for rustup to function correctly.
///
/// # Panics
///
/// Panics if the isolated environment cannot be created or initialised.
#[cfg(unix)]
pub fn setup_isolated_rustup() -> IsolatedRustupEnv {
    let rustup_home = TempDir::new().expect("failed to create RUSTUP_HOME temp dir");
    let cargo_home = TempDir::new().expect("failed to create CARGO_HOME temp dir");

    // Initialise the isolated rustup environment by running `rustup show`.
    // This creates the necessary settings files that rustup expects to exist.
    // We set RUSTUP_AUTO_INSTALL=0 to prevent rustup from auto-installing
    // any toolchain during initialisation.
    // We also clear RUSTUP_TOOLCHAIN and run from the temp directory to avoid
    // rust-toolchain.toml files affecting the initialisation.
    let init_output = Command::new("rustup")
        .arg("show")
        .current_dir(rustup_home.path())
        .env("RUSTUP_HOME", rustup_home.path())
        .env("CARGO_HOME", cargo_home.path())
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .expect("failed to initialise isolated rustup environment");

    assert!(
        init_output.status.success(),
        "failed to initialise isolated rustup: {}",
        String::from_utf8_lossy(&init_output.stderr)
    );

    // Rustup expects to find itself at $CARGO_HOME/bin/rustup. Create a symlink
    // to the system rustup so that toolchain install succeeds.
    let cargo_bin = cargo_home.path().join("bin");
    std::fs::create_dir_all(&cargo_bin).expect("failed to create CARGO_HOME/bin");
    let rustup_path_output = Command::new("which")
        .arg("rustup")
        .output()
        .expect("failed to run which rustup");
    let rustup_path = String::from_utf8_lossy(&rustup_path_output.stdout)
        .trim()
        .to_string();
    std::os::unix::fs::symlink(&rustup_path, cargo_bin.join("rustup"))
        .expect("failed to symlink rustup to CARGO_HOME/bin");

    IsolatedRustupEnv {
        rustup_home,
        cargo_home,
    }
}

/// Sets up isolated RUSTUP_HOME and CARGO_HOME directories for testing (Windows).
///
/// This ensures the auto-install code path is exercised regardless of host state.
/// On Windows, we copy the rustup binary instead of creating a symlink.
///
/// # Panics
///
/// Panics if the isolated environment cannot be created or initialised.
#[cfg(windows)]
pub fn setup_isolated_rustup() -> IsolatedRustupEnv {
    let rustup_home = TempDir::new().expect("failed to create RUSTUP_HOME temp dir");
    let cargo_home = TempDir::new().expect("failed to create CARGO_HOME temp dir");

    // Initialise the isolated rustup environment by running `rustup show`.
    // This creates the necessary settings files that rustup expects to exist.
    // We set RUSTUP_AUTO_INSTALL=0 to prevent rustup from auto-installing
    // any toolchain during initialisation.
    // We also clear RUSTUP_TOOLCHAIN and run from the temp directory to avoid
    // rust-toolchain.toml files affecting the initialisation.
    let init_output = Command::new("rustup")
        .arg("show")
        .current_dir(rustup_home.path())
        .env("RUSTUP_HOME", rustup_home.path())
        .env("CARGO_HOME", cargo_home.path())
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .output()
        .expect("failed to initialise isolated rustup environment");

    assert!(
        init_output.status.success(),
        "failed to initialise isolated rustup: {}",
        String::from_utf8_lossy(&init_output.stderr)
    );

    // Rustup expects to find itself at $CARGO_HOME/bin/rustup.exe. Copy the
    // system rustup binary so that toolchain install succeeds.
    let cargo_bin = cargo_home.path().join("bin");
    std::fs::create_dir_all(&cargo_bin).expect("failed to create CARGO_HOME/bin");
    let rustup_path_output = Command::new("where")
        .arg("rustup")
        .output()
        .expect("failed to run where rustup");
    let rustup_path = String::from_utf8_lossy(&rustup_path_output.stdout)
        .lines()
        .next()
        .expect("rustup not found in PATH")
        .trim()
        .to_string();
    std::fs::copy(&rustup_path, cargo_bin.join("rustup.exe"))
        .expect("failed to copy rustup to CARGO_HOME/bin");

    IsolatedRustupEnv {
        rustup_home,
        cargo_home,
    }
}
