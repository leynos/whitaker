//! Driving the installed bootstrap through `whitaker-installer run`.
//!
//! Adds the run-path helpers to the installation in `bootstrap_install`: the
//! command that runs the installer, the cold-build count, relocation, and
//! fixture Git. Only targets that drive the run path include this module, so
//! the installation module stays free of helpers they would not use.

use std::process::{Command, Output};

use camino::Utf8Path;
use whitaker_installer::test_utils::fixture_git;

pub use crate::bootstrap_install::{Installation, TOOLCHAIN};

impl Installation {
    /// Move the whole data root, as relocating a home directory would.
    ///
    /// # Errors
    ///
    /// Returns an error if the data root cannot be moved.
    pub fn relocate(&mut self, to: &str) -> Result<(), Box<dyn std::error::Error>> {
        let destination = self.base().join(to);
        std::fs::rename(&self.data_root, &destination)?;
        self.data_root = destination;
        Ok(())
    }

    /// A `whitaker-installer run -- <args>` command against this installation.
    pub fn run_command(&self, args: &[&str]) -> Command {
        let base = self.base();
        let path = format!(
            "{}:{}",
            base.join("bin"),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
        command
            .arg("run")
            .arg("--")
            .args(args)
            .env("XDG_DATA_HOME", &self.data_root)
            .env("HOME", base.join("home"))
            .env("PATH", path)
            .env("STUB_LOG", base.join("stub"))
            // Stale values the bootstrap must overwrite rather than trust.
            .env("DYLINT_CLIPPY_REPO", "/stale/clippy")
            .env("DYLINT_DRIVER_PATH", "/stale/drivers")
            .env("DYLINT_LIBRARY_PATH", "/stale/lib")
            // The stub builds where `DriverSlot` looks: under this toolchain.
            .env("STUB_TOOLCHAIN", TOOLCHAIN);
        command
    }

    /// Run with `args` and wait for the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the installer cannot be launched or awaited.
    pub fn run(&self, args: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
        Ok(self.run_command(args).output()?)
    }

    /// How many cold driver builds the stub has performed.
    pub fn builds(&self) -> usize {
        std::fs::read_to_string(self.base().join("stub.builds"))
            .map_or(0, |log| log.lines().count())
    }
}

/// Run fixture Git, returning its output.
///
/// # Errors
///
/// Returns an error if Git cannot be run or exits unsuccessfully.
pub fn git(dir: &Utf8Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    Ok(fixture_git(dir, args)?)
}
