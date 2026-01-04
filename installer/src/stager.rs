//! File staging and platform-specific logic.
//!
//! This module handles copying built libraries to the target directory with
//! the toolchain-specific naming convention required by Dylint.

use crate::builder::{BuildResult, CrateName, library_extension, library_prefix};
use crate::error::{InstallerError, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

/// Handles staging of built libraries to the target directory.
pub struct Stager {
    target_dir: Utf8PathBuf,
    toolchain: String,
}

impl Stager {
    /// Create a new stager with the given target directory and toolchain.
    #[must_use]
    pub fn new(target_dir: Utf8PathBuf, toolchain: &str) -> Self {
        Self {
            target_dir,
            toolchain: toolchain.to_owned(),
        }
    }

    /// Ensure the target directory exists and is writable.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created or is not writable.
    pub fn prepare(&self) -> Result<()> {
        let staging_dir = self.staging_path();

        fs::create_dir_all(&staging_dir)?;

        // Verify writability by attempting to create a temp file
        let test_path = staging_dir.join(".whitaker-installer-test");
        match fs::write(&test_path, b"test") {
            Ok(()) => {
                let _ = fs::remove_file(&test_path);
                Ok(())
            }
            Err(e) => Err(InstallerError::TargetNotWritable {
                path: staging_dir,
                reason: e.to_string(),
            }),
        }
    }

    /// Stage a built library to the target directory.
    ///
    /// The library is copied with the toolchain suffix in its filename,
    /// following the Dylint naming convention.
    ///
    /// # Errors
    ///
    /// Returns an error if the copy operation fails.
    pub fn stage(&self, build_result: &BuildResult) -> Result<Utf8PathBuf> {
        let staged_name = self.staged_filename(&build_result.crate_name);
        let dest_path = self.staging_path().join(&staged_name);

        fs::copy(&build_result.library_path, &dest_path).map_err(|e| {
            InstallerError::StagingFailed {
                reason: format!(
                    "failed to copy {} to {}: {e}",
                    build_result.library_path, dest_path
                ),
            }
        })?;

        Ok(dest_path)
    }

    /// Stage all built libraries.
    ///
    /// # Errors
    ///
    /// Returns an error if any staging operation fails.
    pub fn stage_all(&self, build_results: &[BuildResult]) -> Result<Vec<Utf8PathBuf>> {
        build_results.iter().map(|r| self.stage(r)).collect()
    }

    /// Return the full path to the staging directory.
    #[must_use]
    pub fn staging_path(&self) -> Utf8PathBuf {
        self.target_dir.join(&self.toolchain).join("release")
    }

    /// Return the target directory root.
    #[must_use]
    pub fn target_dir(&self) -> &Utf8Path {
        &self.target_dir
    }

    /// Compute the staged filename with toolchain suffix.
    ///
    /// The filename follows the Dylint naming convention:
    /// `{prefix}{crate_name}@{toolchain}{extension}`
    ///
    /// Where:
    /// - `prefix` is platform-specific (`lib` on Unix, empty on Windows)
    /// - `crate_name` has hyphens replaced with underscores
    /// - `toolchain` is the Rust toolchain channel
    /// - `extension` is platform-specific (`.so`, `.dylib`, or `.dll`)
    #[must_use]
    pub fn staged_filename(&self, crate_name: &CrateName) -> String {
        let base_name = crate_name.as_str().replace('-', "_");
        format!(
            "{}{}@{}{}",
            library_prefix(),
            base_name,
            self.toolchain,
            library_extension()
        )
    }
}

/// Return the default staging directory for the current platform.
///
/// The base directory comes from `dirs::data_local_dir()`, which resolves to a
/// per-user, platform-specific local data directory (for example,
/// `~/.local/share` on many Linux distributions, `~/Library/Application Support`
/// on macOS, and the Local AppData directory on Windows). The installer appends
/// `dylint/lib` under that directory.
#[must_use]
pub fn default_target_dir() -> Option<Utf8PathBuf> {
    dirs::data_local_dir()
        .and_then(|p| Utf8PathBuf::try_from(p).ok())
        .map(|p| p.join("dylint").join("lib"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_filename_includes_toolchain() {
        let stager = Stager::new(Utf8PathBuf::from("/tmp/test"), "nightly-2025-09-18");
        let crate_name = CrateName::from("module_max_lines");
        let filename = stager.staged_filename(&crate_name);

        assert!(filename.contains("nightly-2025-09-18"));
        assert!(filename.contains("module_max_lines"));
    }

    #[test]
    fn staging_path_includes_toolchain_and_release() {
        let stager = Stager::new(
            Utf8PathBuf::from("/home/user/.local/share/dylint/lib"),
            "nightly-2025-09-18",
        );
        let path = stager.staging_path();

        // Use ends_with for platform-independent path checking (compares components,
        // not strings, so it works with both / and \ separators)
        assert!(path.ends_with("nightly-2025-09-18/release"));
        assert!(path.as_str().contains("dylint"));
        assert!(path.as_str().contains("lib"));
    }

    #[test]
    fn default_target_dir_is_some() {
        // Skip assertion in environments without a home directory (e.g., CI containers)
        let Some(d) = default_target_dir() else {
            return;
        };
        assert!(d.as_str().contains("dylint"));
    }
}
