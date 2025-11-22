use super::{StdFsUsage, UsageCategory, label_is_std_fs};
use rstest::rstest;

#[rstest]
#[case("std::fs", true)]
#[case("std::fs::File::open", true)]
#[case("std::fs::read_to_string", true)]
// Ambiguous/edge cases follow
#[case("std::path::Path", false)]
#[case("cap_std::fs::Dir", false)]
#[case("std::fs_extra", false)]
#[case("std::fs2", false)]
#[case("std::fs ", false)]
#[case(" std::fs", false)]
#[case("std::fs::", true)]
#[case("std::fs::File ::open", false)]
#[case("std::fs::File\t::open", false)]
#[case("std::fs::File::open ", false)]
#[case("std::fs::File::open()", false)]
fn recognises_std_fs_paths(#[case] path: &str, #[case] expected: bool) {
    assert_eq!(label_is_std_fs(path), expected);
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
