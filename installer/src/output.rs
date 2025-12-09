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

    #[test]
    fn snippet_contains_path() {
        let path = Utf8PathBuf::from("/home/user/.local/share/dylint/lib");
        let snippet = ShellSnippet::new(&path);

        assert!(snippet.bash.contains("/home/user/.local/share/dylint/lib"));
        assert!(snippet.fish.contains("/home/user/.local/share/dylint/lib"));
        assert!(
            snippet
                .powershell
                .contains("/home/user/.local/share/dylint/lib")
        );
    }

    #[test]
    fn bash_snippet_uses_export() {
        let path = Utf8PathBuf::from("/tmp/dylint");
        let snippet = ShellSnippet::new(&path);

        assert!(snippet.bash.starts_with("export "));
        assert!(snippet.bash.contains("DYLINT_LIBRARY_PATH"));
    }

    #[test]
    fn fish_snippet_uses_set_gx() {
        let path = Utf8PathBuf::from("/tmp/dylint");
        let snippet = ShellSnippet::new(&path);

        assert!(snippet.fish.starts_with("set -gx "));
    }

    #[test]
    fn display_text_includes_all_shells() {
        let path = Utf8PathBuf::from("/tmp/dylint");
        let snippet = ShellSnippet::new(&path);
        let display = snippet.display_text();

        assert!(display.contains("bash/zsh"));
        assert!(display.contains("fish"));
        assert!(display.contains("PowerShell"));
    }

    #[test]
    fn success_message_pluralises_correctly() {
        let path = Utf8PathBuf::from("/tmp");

        let single = success_message(1, &path);
        assert!(single.contains("1 lint library"));

        let multiple = success_message(5, &path);
        assert!(multiple.contains("5 lint libraries"));
    }
}
