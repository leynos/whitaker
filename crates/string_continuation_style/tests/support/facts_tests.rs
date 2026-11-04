//! Tests newline detection and source-path safety decisions.

use camino::{Utf8Path, Utf8PathBuf};
use rustc_span::{NormalizedPos, Pos, RelativeBytePos};
use whitaker_string_literals::rewrite::Newline;

use super::{absolute_path, dominant_newline, is_editable_source};

#[test]
fn dominant_newline_uses_removed_crlf_count_and_ignores_a_bom() {
    let positions = [
        NormalizedPos {
            pos: RelativeBytePos::from_usize(0),
            diff: 3,
        },
        NormalizedPos {
            pos: RelativeBytePos::from_usize(2),
            diff: 4,
        },
        NormalizedPos {
            pos: RelativeBytePos::from_usize(4),
            diff: 5,
        },
    ];

    assert_eq!(dominant_newline("a\nb\nc\n", &positions), Newline::CrLf);
    assert_eq!(dominant_newline("a\nb\nc\nd\ne\n", &positions), Newline::Lf);
}

#[test]
fn relative_target_paths_are_anchored_at_the_workspace_root() {
    let root = Utf8Path::new("/workspace");
    assert_eq!(
        absolute_path(root, Utf8PathBuf::from("target")),
        root.join("target")
    );
    assert_eq!(
        absolute_path(root, Utf8PathBuf::from("/shared/target")),
        Utf8PathBuf::from("/shared/target")
    );
}

#[test]
fn parent_components_are_resolved_before_workspace_containment_checks() {
    let root = Utf8Path::new("/workspace");

    assert_eq!(
        absolute_path(root, Utf8PathBuf::from("/workspace/../outside.rs")),
        Utf8PathBuf::from("/outside.rs")
    );
}

#[test]
fn paths_outside_the_working_tree_are_not_editable() {
    assert!(!is_editable_source(Utf8PathBuf::from("/etc/generated.rs")));
}

#[test]
fn the_workspace_target_directory_is_never_editable() {
    let workspace_root = Utf8PathBuf::try_from(
        std::env::current_dir().expect("the test process has a working directory"),
    )
    .expect("the test process path should be UTF-8");
    assert!(!is_editable_source(
        workspace_root.join("target/generated.rs")
    ));
}
