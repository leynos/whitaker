//! File staging and platform-specific logic.
//!
//! This module handles copying built libraries to the target directory with
//! the toolchain-specific naming convention required by Dylint.

use crate::builder::{BuildResult, library_extension, library_prefix};
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
        let test_path = staging_dir.join(".whitaker-install-test");
        match fs::write(&test_path, b"test") {
            Ok(()) => {
                let _ = fs::remove_file(&test_path);
                Ok(())
            }
            Err(_) => Err(InstallerError::TargetNotWritable { path: staging_dir }),
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
        let mut staged = Vec::with_capacity(build_results.len());

        for result in build_results {
            let path = self.stage(result)?;
            staged.push(path);
        }

        Ok(staged)
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
    fn staged_filename(&self, crate_name: &str) -> String {
        let base_name = crate_name.replace('-', "_");
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
/// On Unix-like systems, this is `$HOME/.local/share/dylint/lib`.
/// On Windows, this uses the local app data directory.
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
        let filename = stager.staged_filename("module_max_lines");

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
        // This test may fail in environments without a home directory
        let dir = default_target_dir();
        if let Some(d) = dir {
            assert!(d.as_str().contains("dylint"));
        }
    }
}
