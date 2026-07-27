//! Module-path scoped suppression for the `no_std_fs_operations` lint.
//!
//! Whereas `excluded_crates` exempts an entire crate, `excluded_paths` exempts
//! individual modules (and everything nested beneath them). The matching logic
//! here is deliberately free of any `rustc` dependency so it can be exercised by
//! ordinary unit and behavioural tests; the driver supplies the enclosing item
//! path resolved from the HIR.

use std::collections::HashSet;
use whitaker_common::SimplePath;

/// Set of module-path prefixes whose items are exempt from the lint.
///
/// Each configured entry is a fully qualified path anchored at the crate
/// identifier (for example `my_app::legacy_io`). An item is excluded when one
/// of the configured paths is a *segment-wise* prefix of the item's own path,
/// so `my_app::legacy_io` exempts `my_app::legacy_io` itself and everything
/// nested beneath it, but never a sibling such as `my_app::legacy_io_utils`.
///
/// Segment-wise matching is what distinguishes this from a naive string prefix
/// test: `a::b` must not be treated as a prefix of `a::bc`.
#[derive(Clone, Debug, Default)]
pub(crate) struct PathExclusions {
    prefixes: Vec<SimplePath>,
}

impl PathExclusions {
    /// Build the exclusion set from configured path strings.
    ///
    /// Malformed entries are rejected before parsing, not compacted: an empty
    /// string, a bare `::`, or a syntactically incomplete path such as
    /// `my_app::` (leading, trailing, or repeated separators) is discarded. This
    /// matters because `SimplePath::parse` silently drops empty segments, so
    /// `my_app::` would otherwise collapse to the crate-root prefix `my_app` and
    /// suppress the lint across the whole crate — the opposite of the narrow,
    /// module-scoped exclusion the entry was meant to express.
    pub(crate) fn new(paths: &HashSet<String>) -> Self {
        let mut prefixes: Vec<SimplePath> = paths
            .iter()
            .filter(|path| is_well_formed_path(path.as_str()))
            .map(|path| SimplePath::parse(path))
            .collect();
        // A deterministic order keeps behaviour reproducible across runs and
        // makes any debug logging of the prefixes stable.
        prefixes.sort_by(|left, right| left.segments().cmp(right.segments()));
        Self { prefixes }
    }

    /// Returns `true` when no path exclusions are configured.
    ///
    /// The driver consults this before resolving an item's path so the common
    /// case pays no lookup cost.
    pub(crate) fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }

    /// Returns `true` when `item_path` falls within a configured exclusion.
    ///
    /// `item_path` is the fully qualified path of the item enclosing a detected
    /// `std::fs` usage.
    pub(crate) fn excludes(&self, item_path: &SimplePath) -> bool {
        let item = item_path.segments();
        self.prefixes.iter().any(|prefix| {
            let prefix = prefix.segments();
            prefix.len() <= item.len() && item[..prefix.len()] == *prefix
        })
    }
}

/// Returns `true` when `path` is a syntactically complete `::`-delimited path.
///
/// A well-formed entry has one or more non-empty segments and no leading,
/// trailing, or repeated separators. A bare crate name (a single segment) is
/// well formed and legitimately exempts the whole crate; only entries whose raw
/// structure would lose a segment during parsing are rejected.
fn is_well_formed_path(path: &str) -> bool {
    !path.is_empty() && path.split("::").all(|segment| !segment.is_empty())
}

#[cfg(test)]
mod tests {
    use super::PathExclusions;
    use rstest::rstest;
    use std::collections::HashSet;
    use whitaker_common::SimplePath;

    fn exclusions(paths: &[&str]) -> PathExclusions {
        PathExclusions::new(
            &paths
                .iter()
                .map(|p| (*p).to_owned())
                .collect::<HashSet<_>>(),
        )
    }

    #[test]
    fn empty_configuration_reports_empty() {
        assert!(exclusions(&[]).is_empty());
    }

    #[test]
    fn configuration_of_only_blank_entries_reports_empty() {
        // Blank and separator-only entries are rejected as malformed, so they
        // never disable the lint wholesale.
        assert!(exclusions(&["", "::"]).is_empty());
    }

    #[test]
    fn populated_configuration_reports_non_empty() {
        assert!(!exclusions(&["my_app::legacy_io"]).is_empty());
    }

    #[rstest]
    #[case::exact_match(&["my_app::legacy_io"], "my_app::legacy_io", true)]
    #[case::nested_child(&["my_app::legacy_io"], "my_app::legacy_io::reader", true)]
    #[case::deeply_nested(&["my_app::legacy_io"], "my_app::legacy_io::reader::inner", true)]
    #[case::sibling_prefix_does_not_match(&["my_app::legacy_io"], "my_app::legacy_io_utils", false)]
    #[case::unrelated_module(&["my_app::legacy_io"], "my_app::network", false)]
    #[case::shorter_item_than_prefix(&["my_app::legacy_io"], "my_app", false)]
    #[case::crate_root_prefix_matches_everything(&["my_app"], "my_app::network::client", true)]
    #[case::multiple_prefixes_first(&["my_app::a", "my_app::b"], "my_app::a::inner", true)]
    #[case::multiple_prefixes_second(&["my_app::a", "my_app::b"], "my_app::b", true)]
    #[case::multiple_prefixes_none(&["my_app::a", "my_app::b"], "my_app::c", false)]
    #[case::no_exclusions(&[], "my_app::anything", false)]
    fn excludes_matches_on_segment_boundaries(
        #[case] configured: &[&str],
        #[case] item: &str,
        #[case] expected: bool,
    ) {
        let exclusions = exclusions(configured);
        assert_eq!(exclusions.excludes(&SimplePath::parse(item)), expected);
    }

    #[test]
    fn blank_entries_do_not_exclude_arbitrary_items() {
        // Regression guard: a stray empty entry must not turn into a wildcard.
        let exclusions = exclusions(&["", "my_app::legacy_io"]);
        assert!(!exclusions.excludes(&SimplePath::parse("other_app::network")));
        assert!(exclusions.excludes(&SimplePath::parse("my_app::legacy_io::reader")));
    }

    #[rstest]
    #[case::trailing_separator("my_app::")]
    #[case::leading_separator("::my_app")]
    #[case::repeated_separator("my_app::::legacy_io")]
    #[case::only_separators("::")]
    #[case::empty("")]
    fn malformed_entries_are_rejected(#[case] entry: &str) {
        // A malformed entry must be dropped rather than compacted; otherwise a
        // trailing `::` would widen `my_app::` into the crate-root prefix
        // `my_app` and disable the lint across the whole crate.
        assert!(
            exclusions(&[entry]).is_empty(),
            "entry {entry:?} should be rejected"
        );
    }

    #[test]
    fn incomplete_path_does_not_widen_to_crate_root() {
        // Regression guard for the specific `my_app::` widening hazard: it must
        // not suppress unrelated items in `my_app`.
        let exclusions = exclusions(&["my_app::"]);
        assert!(!exclusions.excludes(&SimplePath::parse("my_app::network")));
        assert!(!exclusions.excludes(&SimplePath::parse("my_app")));
    }
}
