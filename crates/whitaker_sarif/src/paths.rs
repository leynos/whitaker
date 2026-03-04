//! Stable file layout helpers for the clone detection workflow.
//!
//! The clone detector writes SARIF files to a well-known location under the
//! Cargo target directory. This module provides constants and path-building
//! functions that keep the layout consistent across the CLI, merge logic, and
//! Dylint lint consumer.

use camino::{Utf8Path, Utf8PathBuf};

/// Subdirectory under `target/` for Whitaker clone detection artefacts.
pub const WHITAKER_DIR: &str = "whitaker";

/// Filename for the token-pass SARIF output (Run 0).
pub const TOKEN_PASS_FILENAME: &str = "clones.token.sarif";

/// Filename for the AST-pass SARIF output (Run 1).
pub const AST_PASS_FILENAME: &str = "clones.ast.sarif";

/// Filename for the refined (merged) SARIF output.
pub const REFINED_FILENAME: &str = "clones.refined.sarif";

/// Returns the Whitaker artefact directory under the given target directory.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use whitaker_sarif::whitaker_dir;
///
/// let dir = whitaker_dir(Utf8Path::new("target"));
/// assert_eq!(dir.as_str(), "target/whitaker");
/// ```
#[must_use]
pub fn whitaker_dir(target_dir: &Utf8Path) -> Utf8PathBuf {
    target_dir.join(WHITAKER_DIR)
}

/// Returns the token-pass SARIF file path.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use whitaker_sarif::token_pass_path;
///
/// let path = token_pass_path(Utf8Path::new("target"));
/// assert_eq!(path.as_str(), "target/whitaker/clones.token.sarif");
/// ```
#[must_use]
pub fn token_pass_path(target_dir: &Utf8Path) -> Utf8PathBuf {
    whitaker_dir(target_dir).join(TOKEN_PASS_FILENAME)
}

/// Returns the AST-pass SARIF file path.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use whitaker_sarif::ast_pass_path;
///
/// let path = ast_pass_path(Utf8Path::new("target"));
/// assert_eq!(path.as_str(), "target/whitaker/clones.ast.sarif");
/// ```
#[must_use]
pub fn ast_pass_path(target_dir: &Utf8Path) -> Utf8PathBuf {
    whitaker_dir(target_dir).join(AST_PASS_FILENAME)
}

/// Returns the refined (merged) SARIF file path.
///
/// # Examples
///
/// ```
/// use camino::Utf8Path;
/// use whitaker_sarif::refined_path;
///
/// let path = refined_path(Utf8Path::new("target"));
/// assert_eq!(path.as_str(), "target/whitaker/clones.refined.sarif");
/// ```
#[must_use]
pub fn refined_path(target_dir: &Utf8Path) -> Utf8PathBuf {
    whitaker_dir(target_dir).join(REFINED_FILENAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitaker_dir_appends_subdirectory() {
        let dir = whitaker_dir(Utf8Path::new("/project/target"));
        assert_eq!(dir.as_str(), "/project/target/whitaker");
    }

    #[test]
    fn token_pass_path_is_correct() {
        let path = token_pass_path(Utf8Path::new("/project/target"));
        assert_eq!(path.as_str(), "/project/target/whitaker/clones.token.sarif");
    }

    #[test]
    fn ast_pass_path_is_correct() {
        let path = ast_pass_path(Utf8Path::new("/project/target"));
        assert_eq!(path.as_str(), "/project/target/whitaker/clones.ast.sarif");
    }

    #[test]
    fn refined_path_is_correct() {
        let path = refined_path(Utf8Path::new("/project/target"));
        assert_eq!(
            path.as_str(),
            "/project/target/whitaker/clones.refined.sarif"
        );
    }
}
