//! Output formatting for the installer CLI.
//!
//! This module provides utilities to generate shell configuration snippets
//! that users can add to their shell profile to enable Dylint library discovery,
//! as well as dry-run information formatting.

use crate::crate_name::CrateName;
use camino::Utf8Path;

/// Shell configuration snippets for different shells.
#[derive(Debug, Clone)]
pub struct ShellSnippet {
    /// Export line for bash/zsh.
    pub bash: String,
    /// Set line for fish shell.
    pub fish: String,
    /// Set line for PowerShell.
    pub powershell: String,
}

impl ShellSnippet {
    /// Create shell snippets for the given library path.
    ///
    /// # Example
    ///
    /// ```
    /// use camino::Utf8PathBuf;
    /// use whitaker_installer::output::ShellSnippet;
    ///
    /// let path = Utf8PathBuf::from(
    ///     "/home/user/.local/share/dylint/lib/nightly-2025-01-15/release"
    /// );
    /// let snippet = ShellSnippet::new(&path);
    ///
    /// assert!(snippet.bash.contains("DYLINT_LIBRARY_PATH"));
    /// ```
    #[must_use]
    pub fn new(library_path: &Utf8Path) -> Self {
        Self {
            bash: format!("export DYLINT_LIBRARY_PATH=\"{library_path}\""),
            fish: format!("set -gx DYLINT_LIBRARY_PATH \"{library_path}\""),
            powershell: format!("$env:DYLINT_LIBRARY_PATH = \"{library_path}\""),
        }
    }

    /// Format the snippet for display to the user.
    #[must_use]
    pub fn display_text(&self) -> String {
        format!(
            concat!(
                "Add the following to your shell configuration:\n\n",
                "  # bash/zsh (~/.bashrc, ~/.zshrc)\n",
                "  {}\n\n",
                "  # fish (~/.config/fish/config.fish)\n",
                "  {}\n\n",
                "  # PowerShell ($PROFILE)\n",
                "  {}"
            ),
            self.bash, self.fish, self.powershell
        )
    }
}

/// Format a success message after installation.
#[must_use]
pub fn success_message(count: usize, target_dir: &Utf8Path) -> String {
    let plural = if count == 1 { "library" } else { "libraries" };
    format!("Successfully installed {count} lint {plural} to {target_dir}")
}

/// Configuration information for dry-run output.
///
/// # Example
///
/// ```
/// use camino::Utf8PathBuf;
/// use whitaker_installer::crate_name::CrateName;
/// use whitaker_installer::output::DryRunInfo;
///
/// let workspace = Utf8PathBuf::from("/home/user/whitaker");
/// let target = Utf8PathBuf::from("/home/user/.local/share/dylint/lib");
/// let crates = vec![CrateName::from("suite")];
///
/// let info = DryRunInfo {
///     workspace_root: &workspace,
///     toolchain: "nightly-2025-01-15",
///     target_dir: &target,
///     verbosity: 0,
///     quiet: false,
///     skip_deps: false,
///     skip_wrapper: false,
///     no_update: false,
///     jobs: None,
///     crates: &crates,
/// };
///
/// let output = info.display_text();
/// assert!(output.contains("Dry run"));
/// assert!(output.contains("suite"));
/// ```
#[derive(Debug)]
pub struct DryRunInfo<'a> {
    /// Path to the workspace root.
    pub workspace_root: &'a Utf8Path,
    /// Toolchain channel string.
    pub toolchain: &'a str,
    /// Target directory for staged libraries.
    pub target_dir: &'a Utf8Path,
    /// Verbosity level (0 = normal, 1+ = verbose).
    pub verbosity: u8,
    /// Whether quiet mode is enabled.
    pub quiet: bool,
    /// Whether dependency installation is skipped.
    pub skip_deps: bool,
    /// Whether wrapper script generation is skipped.
    pub skip_wrapper: bool,
    /// Whether repository updates are disabled.
    pub no_update: bool,
    /// Optional parallel job count.
    pub jobs: Option<usize>,
    /// Crates to be built.
    pub crates: &'a [CrateName],
}

impl DryRunInfo<'_> {
    /// Format the dry-run information for display.
    #[must_use]
    pub fn display_text(&self) -> String {
        let mut lines = vec![
            "Dry run - no files will be modified".to_owned(),
            String::new(),
            format!("Workspace root: {}", self.workspace_root),
            format!("Toolchain: {}", self.toolchain),
            format!("Target directory: {}", self.target_dir),
            format!("Verbosity level: {}", self.verbosity),
            format!("Quiet: {}", self.quiet),
            format!("Skip deps: {}", self.skip_deps),
            format!("Skip wrapper: {}", self.skip_wrapper),
            format!("No update: {}", self.no_update),
        ];

        if let Some(jobs) = self.jobs {
            lines.push(format!("Parallel jobs: {jobs}"));
        }

        lines.push(String::new());
        lines.push("Crates to build:".to_owned());
        for crate_name in self.crates {
            lines.push(format!("  - {crate_name}"));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use rstest::{fixture, rstest};

    /// Shared fixture providing a test library path.
    #[fixture]
    fn test_path() -> Utf8PathBuf {
        Utf8PathBuf::from("/home/user/.local/share/dylint/lib/nightly-2025-01-15/release")
    }

    /// Shared fixture providing a shell snippet for the test path.
    #[fixture]
    fn test_snippet(test_path: Utf8PathBuf) -> ShellSnippet {
        ShellSnippet::new(&test_path)
    }

    #[rstest]
    fn snippet_contains_path(test_snippet: ShellSnippet, test_path: Utf8PathBuf) {
        let path_str = test_path.as_str();
        assert!(test_snippet.bash.contains(path_str));
        assert!(test_snippet.fish.contains(path_str));
        assert!(test_snippet.powershell.contains(path_str));
    }

    #[rstest]
    fn bash_snippet_uses_export(test_snippet: ShellSnippet) {
        assert!(test_snippet.bash.starts_with("export "));
        assert!(test_snippet.bash.contains("DYLINT_LIBRARY_PATH"));
    }

    #[rstest]
    fn fish_snippet_uses_set_gx(test_snippet: ShellSnippet) {
        assert!(test_snippet.fish.starts_with("set -gx "));
    }

    #[rstest]
    fn display_text_includes_all_shells(test_snippet: ShellSnippet) {
        let display = test_snippet.display_text();

        assert!(display.contains("bash/zsh"));
        assert!(display.contains("fish"));
        assert!(display.contains("PowerShell"));
    }

    #[rstest]
    #[case::singular(1, "1 lint library")]
    #[case::plural(5, "5 lint libraries")]
    fn success_message_pluralises_correctly(#[case] count: usize, #[case] expected: &str) {
        let path = Utf8PathBuf::from("/tmp");
        let msg = success_message(count, &path);
        assert!(msg.contains(expected));
    }
}
