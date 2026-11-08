//! Tests for malformed-entry rejection and segment-wise prefix matching.

use rstest::rstest;
use whitaker_common::SimplePath;

use super::PathExclusions;

fn exclusions(paths: &[&str]) -> PathExclusions {
    PathExclusions::new(&paths.iter().map(|path| (*path).to_owned()).collect())
}

#[rstest]
#[case::trailing_separator("my_app::")]
#[case::leading_separator("::my_app")]
#[case::repeated_separator("my_app::::legacy")]
#[case::only_separators("::")]
#[case::empty("")]
fn malformed_entries_are_rejected(#[case] entry: &str) {
    assert!(!exclusions(&[entry]).excludes(&SimplePath::parse("my_app::legacy")));
}

#[rstest]
#[case::exact(&["my_app::legacy"], "my_app::legacy", true)]
#[case::nested(&["my_app::legacy"], "my_app::legacy::inner", true)]
#[case::crate_root(&["my_app"], "my_app::network", true)]
#[case::sibling_prefix(&["my_app::legacy"], "my_app::legacy_utils", false)]
#[case::unrelated(&["my_app::legacy"], "my_app::network", false)]
#[case::no_exclusions(&[], "my_app::anything", false)]
fn excludes_matches_on_segment_boundaries(
    #[case] configured: &[&str],
    #[case] module_path: &str,
    #[case] expected: bool,
) {
    assert_eq!(
        exclusions(configured).excludes(&SimplePath::parse(module_path)),
        expected
    );
}

#[test]
fn incomplete_path_does_not_widen_to_crate_root() {
    let excluded = exclusions(&["my_app::"]);
    assert!(!excluded.excludes(&SimplePath::parse("my_app::network")));
}

#[test]
fn long_multibyte_entries_truncate_on_a_character_boundary() {
    let entry = "€".repeat(100);
    let rendered = super::bounded_entry(&entry);
    assert!(rendered.contains("bytes total"), "rendered: {rendered}");
    assert!(rendered.len() < entry.len(), "rendered: {rendered}");
}
