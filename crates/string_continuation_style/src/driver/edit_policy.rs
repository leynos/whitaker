//! Resolves which source paths the lint may offer a rewrite against.
//!
//! The lint only suggests an edit for a file the user can actually edit. A
//! literal that came from the target directory, or from outside the workspace
//! altogether, is generated rather than authored, and rewriting it would edit
//! output the user does not maintain.
//!
//! Resolution happens once, at the compiler boundary in `check_crate`. The
//! environment is read there and nowhere else, so the per-literal query that
//! runs for every candidate is pure and testable against fixed directories.

use std::env;

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use log::debug;

use crate::config::LINT_NAME;

/// The directories a rewrite may and may not be offered against.
///
/// Constructing one is the only place that reads the process environment.
/// Nothing downstream re-reads `CARGO_TARGET_DIR` or the current directory,
/// which keeps the editability query a function of its inputs.
pub(crate) struct SourceEditPolicy {
    workspace_root: Utf8PathBuf,
    target_directories: Vec<Utf8PathBuf>,
}

impl SourceEditPolicy {
    /// Resolves the workspace root and target directories from `env`.
    ///
    /// Fails closed in every uncertain case, and logs why. A literal is not
    /// worth a diagnostic if the answer is "cannot tell whether it is
    /// editable"; suppressing the finding costs the user a hint, while guessing
    /// wrong offers an edit to generated code.
    ///
    /// # Errors
    ///
    /// Returns `None` when the current directory is unavailable or not UTF-8,
    /// or when `CARGO_TARGET_DIR` is set to a non-UTF-8 value.
    pub(crate) fn from_env() -> Option<Self> {
        let Some(workspace_root) = env::current_dir()
            .ok()
            .and_then(|path| Utf8PathBuf::try_from(path).ok())
        else {
            debug!(
                target: LINT_NAME,
                "cannot resolve the workspace root; treating every literal as generated"
            );
            return None;
        };
        Some(Self::new(workspace_root, env::var("CARGO_TARGET_DIR").ok()))
    }

    /// Builds a policy from an explicit workspace root and target directory.
    ///
    /// The `target_directory` argument is the value of `CARGO_TARGET_DIR`, if
    /// set. The default `target` directory beneath the workspace root is always
    /// excluded as well, whether or not an override is configured, so a
    /// configured directory never re-admits the default one.
    pub(crate) fn new(workspace_root: Utf8PathBuf, target_directory: Option<String>) -> Self {
        let default_directory = workspace_root.join("target");
        let configured =
            target_directory.map_or_else(|| default_directory.clone(), Utf8PathBuf::from);
        let mut target_directories = vec![absolute_path(&workspace_root, configured)];
        if !target_directories.contains(&default_directory) {
            target_directories.push(default_directory);
        }
        Self {
            workspace_root,
            target_directories,
        }
    }

    /// Reports whether a rewrite may be offered for `source_path`.
    ///
    /// A relative path is anchored at the workspace root, because the source
    /// map reports the path the compiler was invoked with rather than an
    /// absolute one. Containment is then tested against the resolved path, so
    /// `../` segments cannot walk out of the tree unnoticed.
    pub(crate) fn is_editable(&self, source_path: &Utf8Path) -> bool {
        let resolved = absolute_path(&self.workspace_root, source_path.to_path_buf());
        resolved.starts_with(&self.workspace_root)
            && !self
                .target_directories
                .iter()
                .any(|directory| resolved.starts_with(directory))
    }
}

/// Resolves a possibly-relative path against `root` and normalizes it.
fn absolute_path(root: &Utf8Path, path: Utf8PathBuf) -> Utf8PathBuf {
    let anchored = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let mut normalized = Utf8PathBuf::new();
    for component in anchored.components() {
        match component {
            Utf8Component::Prefix(_) | Utf8Component::RootDir => {
                normalized.push(component.as_str());
            }
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                normalized.pop();
            }
            Utf8Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

#[cfg(test)]
#[path = "../../tests/support/edit_policy_tests.rs"]
mod tests;
