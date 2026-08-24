//! Workspace-root lookup for repository-configuration integration tests.
//!
//! This module is included only by tests that inspect repository-level files.
//! Production inputs must be supplied explicitly by callers instead.

use std::path::{Path, PathBuf};

/// Returns the workspace root for the nested core crate's integration tests.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map_or_else(
            || panic!("whitaker_lint_core must remain nested under crates/"),
            Path::to_path_buf,
        )
}
