//! Module-path scoped suppression for the `string_continuation_style` lint.
//!
//! The early lint pass knows the module path it is walking but not the fully
//! qualified item path a late pass would see, so `excluded_paths` entries are
//! matched against the crate identifier followed by the enclosing modules.
//!
//! The matching logic deliberately avoids `rustc` types so it can be exercised
//! by ordinary unit tests; the driver supplies the enclosing module segments.

use std::collections::HashSet;

use log::warn;
use whitaker_common::SimplePath;

use crate::config::LINT_NAME;

/// Maximum length of a malformed entry echoed into a warning, so a pathological
/// configuration value cannot produce an unbounded log line.
const MAX_LOGGED_ENTRY_LEN: usize = 64;

/// Set of module-path prefixes whose literals are exempt from the lint.
///
/// Each configured entry is a fully qualified path anchored at the crate
/// identifier (for example `my_app::legacy_text`). A literal is excluded when
/// one configured path is a segment-wise prefix of its own path, so
/// `my_app::legacy_text` exempts that module and everything nested beneath it,
/// but never a sibling such as `my_app::legacy_text_utils`.
#[derive(Clone, Debug, Default)]
pub(crate) struct PathExclusions {
    prefixes: Vec<SimplePath>,
}

impl PathExclusions {
    /// Builds the exclusion set from configured path strings.
    ///
    /// Malformed entries are rejected before parsing, not compacted. This
    /// matters because [`SimplePath::parse`] silently drops empty segments, so
    /// `my_app::` would otherwise collapse to the crate-root prefix `my_app`
    /// and suppress the lint across the whole crate.
    pub(crate) fn new(paths: &HashSet<String>) -> Self {
        let mut prefixes: Vec<SimplePath> = Vec::with_capacity(paths.len());
        for path in paths {
            if is_well_formed_path(path) {
                prefixes.push(SimplePath::parse(path));
            } else {
                warn!(
                    target: LINT_NAME,
                    "ignoring malformed `excluded_paths` entry {}",
                    bounded_entry(path)
                );
            }
        }
        prefixes.sort_by(|left, right| left.segments().cmp(right.segments()));
        Self { prefixes }
    }

    /// Returns `true` when `module_path` falls within a configured exclusion.
    pub(crate) fn excludes(&self, module_path: &SimplePath) -> bool {
        let segments = module_path.segments();
        self.prefixes
            .iter()
            .any(|prefix| segments.starts_with(prefix.segments()))
    }
}

/// Returns `true` when `path` is a syntactically complete `::`-delimited path.
///
/// A bare crate name is well formed and legitimately exempts the whole crate;
/// only entries whose raw structure would lose a segment during parsing are
/// rejected.
fn is_well_formed_path(path: &str) -> bool {
    !path.is_empty() && path.split("::").all(|segment| !segment.is_empty())
}

/// Renders a malformed entry for a warning: quoted, so control characters are
/// escaped, and truncated to a bounded length.
fn bounded_entry(entry: &str) -> String {
    if entry.len() <= MAX_LOGGED_ENTRY_LEN {
        return format!("{entry:?}");
    }
    let mut end = MAX_LOGGED_ENTRY_LEN;
    while !entry.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    let truncated = entry.get(..end).unwrap_or(entry);
    format!("{truncated:?}… ({} bytes total)", entry.len())
}

#[cfg(test)]
#[path = "../tests/support/exclusion_tests.rs"]
mod tests;
