//! Wrapper scripts generation for easy Whitaker invocation.
//!
//! This module generates platform-specific `whitaker` and `whitaker-ls`
//! scripts. They hold no paths: each one finds `whitaker-installer` (beside
//! itself first, then on `PATH`) and calls `whitaker-installer run`, which
//! resolves the lint libraries, Clippy snapshot, and driver directory from the
//! current data root, serializes cold driver construction, and returns
//! `cargo dylint`'s exit status. A relocated installation therefore needs no
//! regenerated scripts.

use std::path::Path;

use crate::{
    dirs::BaseDirs,
    error::{InstallerError, Result},
    resolution::SUITE_CRATE,
};

/// Result of wrapper script generation.
#[derive(Debug)]
pub struct WrapperResult {
    /// Conventional path for the `whitaker` wrapper, whether or not that
    /// script was selected for creation.
    pub whitaker_path: std::path::PathBuf,
    /// Path to the `whitaker-ls` wrapper script.
    pub whitaker_ls_path: std::path::PathBuf,
    /// Whether the bin directory is in PATH.
    pub in_path: bool,
}

/// Selects which compatibility scripts an installer invocation may create.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapperScripts {
    /// Write both scripts used by the legacy `whitaker-installer` command.
    Both,
    /// Write no script, because the `whitaker` binary now provides `ls`.
    ///
    /// The generated lister calls `whitaker-installer run`, so writing it from
    /// the root binary would leave a script that fails unless the installer is
    /// installed separately. The root CLI offers the same listing itself.
    None,
}

impl WrapperScripts {
    /// Reports whether this policy belongs to the root binary.
    ///
    /// The root CLI forwards into the recorded staging directory from any
    /// working directory, so it cannot continue after a record failure the way
    /// the legacy installer can.
    #[must_use]
    pub const fn is_root_cli(self) -> bool { matches!(self, Self::None) }
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
/// use whitaker_installer::{
///     dirs::{BaseDirs, SystemBaseDirs},
///     wrapper::generate_wrapper_scripts,
/// };
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
    generate_wrapper_scripts_with(dirs, WrapperScripts::Both)
}

/// Generates the selected compatibility scripts for invoking Whitaker lints.
///
/// The returned paths name the conventional locations even when a script was
/// not selected for writing. This lets callers report a consistent PATH
/// location while preserving an executable installed at the `whitaker` path.
///
/// # Examples
///
/// ```no_run
/// use whitaker_installer::{
///     dirs::SystemBaseDirs,
///     wrapper::{WrapperScripts, generate_wrapper_scripts_with},
/// };
///
/// let dirs = SystemBaseDirs::new().expect("platform directories should exist");
/// let result = generate_wrapper_scripts_with(&dirs, WrapperScripts::None)?;
/// assert!(!result.whitaker_ls_path.exists());
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
///
/// # Errors
///
/// Returns a wrapper-generation error if the bin directory cannot be created
/// or a selected script cannot be written.
pub fn generate_wrapper_scripts_with(
    dirs: &dyn BaseDirs,
    scripts: WrapperScripts,
) -> Result<WrapperResult> {
    let bin_dir = dirs.executables().ok_or_else(|| {
        InstallerError::WrapperGeneration("could not determine bin directory".to_owned())
    })?;

    std::fs::create_dir_all(&bin_dir).map_err(|e| {
        InstallerError::WrapperGeneration(format!("failed to create bin directory: {e}"))
    })?;

    #[cfg(unix)]
    let (whitaker_path, whitaker_ls_path) = generate_unix_scripts(&bin_dir, scripts)?;

    #[cfg(windows)]
    let (whitaker_path, whitaker_ls_path) = generate_windows_scripts(&bin_dir, scripts)?;

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
        "#!/usr/bin/env bash\nset -euo pipefail\n{UNIX_LOCATE_INSTALLER}\nexec \"${{installer}}\" \
         run -- \"$@\"\n"
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

/// Write the Unix wrapper scripts selected by `scripts`.
///
/// The conventional `whitaker` and `whitaker-ls` paths are returned whether or
/// not anything was written, so the caller can report where the wrappers live
/// even when [`WrapperScripts::None`] suppresses generation.
///
/// # Errors
///
/// Returns [`InstallerError::WrapperGeneration`] when a script cannot be
/// written or made executable.
#[cfg(unix)]
fn generate_unix_scripts(
    bin_dir: &Path,
    scripts: WrapperScripts,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let whitaker_path = bin_dir.join("whitaker");
    let whitaker_ls_path = bin_dir.join("whitaker-ls");
    match scripts {
        WrapperScripts::Both => {
            write_unix_script(&whitaker_path, &unix_whitaker_script())?;
            write_unix_script(&whitaker_ls_path, &unix_whitaker_ls_script())?;
        }
        WrapperScripts::None => {}
    }

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

/// Write the PowerShell wrapper scripts selected by `scripts`.
///
/// The conventional `whitaker.ps1` and `whitaker-ls.ps1` paths are returned
/// whether or not anything was written, so the caller can report where the
/// wrappers live even when [`WrapperScripts::None`] suppresses generation.
///
/// # Errors
///
/// Returns [`InstallerError::WrapperGeneration`] when a script cannot be
/// written.
#[cfg(windows)]
fn generate_windows_scripts(
    bin_dir: &Path,
    scripts: WrapperScripts,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let whitaker_path = bin_dir.join("whitaker.ps1");
    let whitaker_ls_path = bin_dir.join("whitaker-ls.ps1");
    match scripts {
        WrapperScripts::Both => {
            std::fs::write(&whitaker_path, windows_whitaker_script()).map_err(|e| {
                InstallerError::WrapperGeneration(format!("failed to write script: {e}"))
            })?;
            std::fs::write(&whitaker_ls_path, windows_ls_script()).map_err(|e| {
                InstallerError::WrapperGeneration(format!("failed to write script: {e}"))
            })?;
        }
        WrapperScripts::None => {}
    }

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
$suite = "{SUITE_CRATE}"
& $installer run -- list --color never | Where-Object {{
    $_ -match ("^\s*" + [regex]::Escape($suite) + "(\s|$)")
}}
exit $LASTEXITCODE
"#
    )
}

/// Checks if a directory is in the PATH environment variable.
fn is_directory_in_path(dir: &Path) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|entry| entry == dir))
}

/// Returns instructions for adding a directory to PATH.
#[must_use]
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
