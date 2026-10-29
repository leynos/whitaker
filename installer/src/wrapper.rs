//! Wrapper scripts generation for easy Whitaker invocation.
//!
//! This module generates platform-specific `whitaker` and `whitaker-ls`
//! scripts. They hold no paths: each one finds `whitaker-installer` (beside
//! itself first, then on `PATH`) and calls `whitaker-installer run`, which
//! resolves the lint libraries, Clippy snapshot, and driver directory from the
//! current data root, serializes cold driver construction, and returns
//! `cargo dylint`'s exit status. A relocated installation therefore needs no
//! regenerated scripts.

use crate::dirs::BaseDirs;
use crate::error::{InstallerError, Result};
use crate::resolution::SUITE_CRATE;
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
/// Windows). `whitaker` forwards its arguments to `whitaker-installer run`,
/// while `whitaker-ls` filters `cargo dylint list` output to the Whitaker
/// suite.
///
/// # Errors
///
/// Returns `InstallerError::WrapperGeneration` if script creation fails.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
/// use whitaker_installer::wrapper::generate_wrapper_scripts;
///
/// let dirs = SystemBaseDirs::new().expect("failed to initialize directories");
/// let result = generate_wrapper_scripts(&dirs)?;
///
/// println!("Script created at: {}", result.whitaker_path.display());
/// if result.in_path {
///     println!("Ready to use: whitaker --all");
/// } else {
///     println!("Add the bin directory to your PATH first");
/// }
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
pub fn generate_wrapper_scripts(dirs: &dyn BaseDirs) -> Result<WrapperResult> {
    let bin_dir = dirs.bin_dir().ok_or_else(|| {
        InstallerError::WrapperGeneration("could not determine bin directory".to_owned())
    })?;

    std::fs::create_dir_all(&bin_dir).map_err(|e| {
        InstallerError::WrapperGeneration(format!("failed to create bin directory: {e}"))
    })?;

    #[cfg(unix)]
    let (whitaker_path, whitaker_ls_path) = generate_unix_scripts(&bin_dir)?;

    #[cfg(windows)]
    let (whitaker_path, whitaker_ls_path) = generate_windows_scripts(&bin_dir)?;

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

/// Bash that sets `installer` to the `whitaker-installer` beside the script,
/// or else the one on `PATH`.
#[cfg(unix)]
const UNIX_LOCATE_INSTALLER: &str = r#"script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if [[ -x "${script_dir}/whitaker-installer" ]]; then
    installer="${script_dir}/whitaker-installer"
elif ! installer=$(command -v whitaker-installer); then
    echo "whitaker: whitaker-installer was not found beside ${script_dir} or on PATH;" \
        "install it with: cargo binstall whitaker-installer" >&2
    exit 127
fi"#;

/// Render the Unix `whitaker` script.
#[cfg(unix)]
fn unix_whitaker_script() -> String {
    format!(
        "#!/usr/bin/env bash\nset -euo pipefail\n{UNIX_LOCATE_INSTALLER}\nexec \"${{installer}}\" run -- \"$@\"\n"
    )
}

/// Render the Unix `whitaker-ls` script.
#[cfg(unix)]
fn unix_whitaker_ls_script() -> String {
    format!(
        concat!(
            "#!/usr/bin/env bash\nset -euo pipefail\n{locate}\n",
            "\"${{installer}}\" run -- list --color never | ",
            "awk -v suite=\"{suite_crate}\" '$0 ~ \"^\" suite \"([[:space:]]|$)\" {{ print }}'\n",
        ),
        locate = UNIX_LOCATE_INSTALLER,
        suite_crate = SUITE_CRATE,
    )
}

#[cfg(unix)]
fn generate_unix_scripts(bin_dir: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let whitaker_path = bin_dir.join("whitaker");
    write_unix_script(&whitaker_path, &unix_whitaker_script())?;

    let whitaker_ls_path = bin_dir.join("whitaker-ls");
    write_unix_script(&whitaker_ls_path, &unix_whitaker_ls_script())?;

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

/// PowerShell that sets `$installer` to the `whitaker-installer.exe` beside
/// the script, or else the one on `PATH`.
#[cfg(any(windows, test))]
const WINDOWS_LOCATE_INSTALLER: &str = r#"$installer = Join-Path $PSScriptRoot 'whitaker-installer.exe'
if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
    $command = Get-Command whitaker-installer -CommandType Application -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if (-not $command) {
        [Console]::Error.WriteLine("whitaker: whitaker-installer was not found beside $PSScriptRoot or on PATH; install it with: cargo binstall whitaker-installer")
        exit 127
    }
    $installer = $command.Source
}"#;

#[cfg(windows)]
fn generate_windows_scripts(bin_dir: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let whitaker_path = bin_dir.join("whitaker.ps1");
    let whitaker_content = windows_whitaker_script();

    std::fs::write(&whitaker_path, whitaker_content)
        .map_err(|e| InstallerError::WrapperGeneration(format!("failed to write script: {e}")))?;

    let whitaker_ls_path = bin_dir.join("whitaker-ls.ps1");
    let whitaker_ls_content = windows_ls_script();

    std::fs::write(&whitaker_ls_path, whitaker_ls_content)
        .map_err(|e| InstallerError::WrapperGeneration(format!("failed to write script: {e}")))?;

    Ok((whitaker_path, whitaker_ls_path))
}

/// Render `whitaker.ps1`: locate the installer, then forward every argument.
///
/// The two lookup branches only choose `$installer`; the one invocation line
/// after them forwards the arguments and status for both.
#[cfg(any(windows, test))]
fn windows_whitaker_script() -> String {
    format!("{WINDOWS_LOCATE_INSTALLER}\n& $installer run -- @args\nexit $LASTEXITCODE\n")
}

/// Render `whitaker-ls.ps1`, which keeps only the suite's `list` line.
///
/// Built on every platform under test so the PowerShell pattern is checked
/// even where PowerShell cannot run.
#[cfg(any(windows, test))]
fn windows_ls_script() -> String {
    format!(
        r#"{WINDOWS_LOCATE_INSTALLER}
$suite = "{suite_crate}"
& $installer run -- list --color never | Where-Object {{
    $_ -match ("^\s*" + [regex]::Escape($suite) + "(\s|$)")
}}
exit $LASTEXITCODE
"#,
        suite_crate = SUITE_CRATE,
    )
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
#[path = "wrapper_tests.rs"]
mod tests;
