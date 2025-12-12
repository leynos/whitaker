//! Shell snippet generation for `DYLINT_LIBRARY_PATH`.
//!
//! This module provides utilities to generate shell configuration snippets
//! that users can add to their shell profile to enable Dylint library discovery.

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
    /// let path = Utf8PathBuf::from("/home/user/.local/share/dylint/lib");
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

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use rstest::{fixture, rstest};

    /// Shared fixture providing a test library path.
    #[fixture]
    fn test_path() -> Utf8PathBuf {
        Utf8PathBuf::from("/home/user/.local/share/dylint/lib")
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
