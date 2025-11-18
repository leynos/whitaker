use super::{StdFsUsage, UsageCategory, matches_std_fs_path};
use rstest::rstest;

#[rstest]
#[case("std::fs", true)]
#[case("std::fs::File::open", true)]
#[case("std::fs::read_to_string", true)]
#[case("std::path::Path", false)]
#[case("cap_std::fs::Dir", false)]
fn recognises_std_fs_paths(#[case] path: &str, #[case] expected: bool) {
    assert_eq!(matches_std_fs_path(path), expected);
}

#[rstest]
fn usage_category_labels_are_stable() {
    assert_eq!(UsageCategory::Import.as_str(), "import");
    assert_eq!(UsageCategory::Type.as_str(), "type");
    assert_eq!(UsageCategory::Call.as_str(), "call");
}

#[rstest]
fn usage_builder_carries_operation() {
    let usage = StdFsUsage::new(String::from("std::fs::read"), UsageCategory::Call);
    assert_eq!(usage.operation(), "std::fs::read");
    assert_eq!(usage.category(), UsageCategory::Call);
}
