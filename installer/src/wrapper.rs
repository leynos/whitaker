//! Wrapper script generation for easy Whitaker invocation.
//!
//! This module generates platform-specific scripts that set the
//! `DYLINT_LIBRARY_PATH` environment variable and invoke `cargo dylint`.

use crate::dirs::BaseDirs;
use crate::error::{InstallerError, Result};
use camino::Utf8Path;
use std::path::Path;

/// Result of wrapper script generation.
#[derive(Debug)]
pub struct WrapperResult {
    /// Path to the `whitaker` wrapper script.
    pub whitaker_path: std::path::PathBuf,
    /// Path to the `whitaker-ls` wrapper script.
    pub whitaker_ls_path: std::path::PathBuf,
    /// Whether the bin directory is in PATH.
    pub in_path: bool,
}

/// Generates wrapper scripts for invoking Whitaker lints.
///
/// Creates `whitaker` and `whitaker-ls` scripts (shell on Unix, PowerShell on
/// Windows). `whitaker` forwards to `cargo dylint`, while `whitaker-ls` filters
/// `cargo dylint list` output to the Whitaker suite.
///
/// # Arguments
///
/// * `dirs` - Directory resolver for platform-specific paths.
/// * `library_path` - Path to the staged lint libraries.
///
/// # Returns
///
/// Information about the generated scripts and PATH status.
///
/// # Errors
///
/// Returns `InstallerError::WrapperGeneration` if script creation fails.
///
/// # Examples
///
/// ```no_run
/// use camino::Utf8Path;
/// use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
/// use whitaker_installer::wrapper::generate_wrapper_scripts;
///
/// let dirs = SystemBaseDirs::new().expect("failed to initialise directories");
/// let library_path = Utf8Path::new("/home/user/.local/share/dylint/lib");
/// let result = generate_wrapper_scripts(&dirs, library_path)?;
///
/// println!("Script created at: {}", result.whitaker_path.display());
/// if result.in_path {
///     println!("Ready to use: whitaker --all");
/// } else {
///     println!("Add the bin directory to your PATH first");
/// }
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
pub fn generate_wrapper_scripts(
    dirs: &dyn BaseDirs,
    library_path: &Utf8Path,
) -> Result<WrapperResult> {
    let bin_dir = dirs.bin_dir().ok_or_else(|| {
        InstallerError::WrapperGeneration("could not determine bin directory".to_owned())
    })?;

    std::fs::create_dir_all(&bin_dir).map_err(|e| {
        InstallerError::WrapperGeneration(format!("failed to create bin directory: {e}"))
    })?;

    #[cfg(unix)]
    let (whitaker_path, whitaker_ls_path) = generate_unix_scripts(&bin_dir, library_path)?;

    #[cfg(windows)]
    let (whitaker_path, whitaker_ls_path) = generate_windows_scripts(&bin_dir, library_path)?;

    #[cfg(not(any(unix, windows)))]
    return Err(InstallerError::WrapperGeneration(
        "unsupported platform".to_owned(),
    ));

    let in_path = is_directory_in_path(&bin_dir);

    Ok(WrapperResult {
        whitaker_path,
        whitaker_ls_path,
        in_path,
    })
}

#[cfg(unix)]
fn generate_unix_scripts(
    bin_dir: &Path,
    library_path: &Utf8Path,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let whitaker_path = bin_dir.join("whitaker");
    let whitaker_content = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
export DYLINT_LIBRARY_PATH="{library_path}"
exec cargo dylint "$@"
"#
    );
    write_unix_script(&whitaker_path, &whitaker_content)?;

    let whitaker_ls_path = bin_dir.join("whitaker-ls");
    let whitaker_ls_content = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
export DYLINT_LIBRARY_PATH="{library_path}"
cargo dylint list | awk '$1 == "whitaker_suite" {{ print }}'
"#
    );
    write_unix_script(&whitaker_ls_path, &whitaker_ls_content)?;

    Ok((whitaker_path, whitaker_ls_path))
}

/// Writes an executable Unix shell script.
#[cfg(unix)]
fn write_unix_script(path: &Path, content: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, content)
        .map_err(|e| InstallerError::WrapperGeneration(format!("failed to write script: {e}")))?;

    // Make executable (rwxr-xr-x)
    let mut perms = std::fs::metadata(path)
        .map_err(|e| InstallerError::WrapperGeneration(format!("failed to read permissions: {e}")))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).map_err(|e| {
        InstallerError::WrapperGeneration(format!("failed to set permissions: {e}"))
    })?;

    Ok(())
}

#[cfg(windows)]
fn generate_windows_scripts(
    bin_dir: &Path,
    library_path: &Utf8Path,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let whitaker_path = bin_dir.join("whitaker.ps1");
    let whitaker_content = format!(
        r#"$env:DYLINT_LIBRARY_PATH = "{library_path}"
cargo dylint @args
"#
    );

    std::fs::write(&whitaker_path, whitaker_content)
        .map_err(|e| InstallerError::WrapperGeneration(format!("failed to write script: {e}")))?;

    let whitaker_ls_path = bin_dir.join("whitaker-ls.ps1");
    let whitaker_ls_content = format!(
        r#"$env:DYLINT_LIBRARY_PATH = "{library_path}"
cargo dylint list | Where-Object {{ $_ -match '^\s*whitaker_suite\s' }}
"#
    );

    std::fs::write(&whitaker_ls_path, whitaker_ls_content)
        .map_err(|e| InstallerError::WrapperGeneration(format!("failed to write script: {e}")))?;

    Ok((whitaker_path, whitaker_ls_path))
}

/// Checks if a directory is in the PATH environment variable.
fn is_directory_in_path(dir: &Path) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|p| p == dir))
        .unwrap_or(false)
}

/// Returns instructions for adding a directory to PATH.
pub fn path_instructions(bin_dir: &Path) -> String {
    #[cfg(unix)]
    {
        format!(
            concat!(
                "Add the following to your shell profile (~/.bashrc or ~/.zshrc):\n",
                "  export PATH=\"{}:$PATH\""
            ),
            bin_dir.display()
        )
    }
    #[cfg(windows)]
    {
        format!(
            concat!(
                "Add the following directory to your PATH:\n",
                "  {}\n\n",
                "Or run in PowerShell:\n",
                "  [Environment]::SetEnvironmentVariable(",
                "\"PATH\", \"$env:PATH;{}\", \"User\")"
            ),
            bin_dir.display(),
            bin_dir.display()
        )
    }
    #[cfg(not(any(unix, windows)))]
    {
        format!("Add {} to your PATH", bin_dir.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn is_directory_in_path_returns_false_for_random_dir() {
        let temp = TempDir::new().expect("failed to create temp dir");
        assert!(!is_directory_in_path(temp.path()));
    }

    #[cfg(unix)]
    #[test]
    fn generate_unix_scripts_create_executables() {
        use camino::Utf8PathBuf;
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("failed to create temp dir");
        let library_path = Utf8PathBuf::from("/tmp/dylint/lib");

        let (whitaker_path, whitaker_ls_path) =
            generate_unix_scripts(temp.path(), &library_path).expect("failed to generate scripts");

        assert!(whitaker_path.exists());
        assert!(whitaker_ls_path.exists());

        let perms = std::fs::metadata(&whitaker_path)
            .expect("failed to read metadata")
            .permissions();
        assert_eq!(perms.mode() & 0o111, 0o111, "script should be executable");

        let whitaker_content =
            std::fs::read_to_string(&whitaker_path).expect("failed to read script");
        assert!(whitaker_content.contains("DYLINT_LIBRARY_PATH"));
        assert!(whitaker_content.contains("cargo dylint"));
        assert!(whitaker_content.contains("$@"));

        let whitaker_ls_content =
            std::fs::read_to_string(&whitaker_ls_path).expect("failed to read script");
        assert!(whitaker_ls_content.contains("DYLINT_LIBRARY_PATH"));
        assert!(whitaker_ls_content.contains("cargo dylint list"));
        assert!(whitaker_ls_content.contains("whitaker_suite"));
    }

    #[test]
    fn path_instructions_contains_directory() {
        let dir = std::path::PathBuf::from("/test/bin");
        let instructions = path_instructions(&dir);
        assert!(instructions.contains("/test/bin"));
    }
}
