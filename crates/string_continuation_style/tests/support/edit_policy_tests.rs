//! Tests source-path safety decisions against fixed, injected directories.
//!
//! Every case constructs its policy from explicit paths. None of them reads
//! the process working directory or `CARGO_TARGET_DIR`, so the suite proves
//! the same thing wherever it runs.

use camino::{Utf8Path, Utf8PathBuf};

use super::SourceEditPolicy;

fn policy(root: &str, target_directory: Option<&str>) -> SourceEditPolicy {
    SourceEditPolicy::new(Utf8PathBuf::from(root), target_directory.map(str::to_owned))
}

#[test]
fn relative_target_paths_are_anchored_at_the_workspace_root() {
    let root = Utf8Path::new("/workspace");
    assert_eq!(
        super::absolute_path(root, Utf8PathBuf::from("target")),
        root.join("target")
    );
    assert_eq!(
        super::absolute_path(root, Utf8PathBuf::from("/shared/target")),
        Utf8PathBuf::from("/shared/target")
    );
}

#[test]
fn parent_components_are_resolved_before_workspace_containment_checks() {
    let root = Utf8Path::new("/workspace");

    assert_eq!(
        super::absolute_path(root, Utf8PathBuf::from("/workspace/../outside.rs")),
        Utf8PathBuf::from("/outside.rs")
    );
    assert!(!policy("/workspace", None).is_editable(Utf8Path::new("/workspace/../outside.rs")));
}

#[test]
fn paths_outside_the_working_tree_are_not_editable() {
    assert!(!policy("/workspace", None).is_editable(Utf8Path::new("/etc/generated.rs")));
}

#[test]
fn workspace_sources_are_editable() {
    let policy = policy("/workspace", None);

    assert!(policy.is_editable(Utf8Path::new("/workspace/src/main.rs")));
    assert!(policy.is_editable(Utf8Path::new("src/main.rs")));
}

#[test]
fn the_default_target_directory_is_never_editable() {
    let policy = policy("/workspace", None);

    assert!(!policy.is_editable(Utf8Path::new("/workspace/target/generated.rs")));
    assert!(!policy.is_editable(Utf8Path::new("target/generated.rs")));
}

#[test]
fn a_configured_target_directory_is_never_editable() {
    let policy = policy("/workspace", Some("/shared/target"));

    assert!(!policy.is_editable(Utf8Path::new("/shared/target/generated.rs")));
    // The override must not re-admit the default directory, which a build can
    // still write to even when `CARGO_TARGET_DIR` points somewhere else.
    assert!(!policy.is_editable(Utf8Path::new("/workspace/target/generated.rs")));
    assert!(policy.is_editable(Utf8Path::new("/workspace/src/main.rs")));
}

#[test]
fn a_relative_configured_target_directory_is_anchored_at_the_root() {
    let policy = policy("/workspace", Some("build/generated"));

    assert!(!policy.is_editable(Utf8Path::new("/workspace/build/generated/out.rs")));
    assert!(policy.is_editable(Utf8Path::new("/workspace/build/source.rs")));
}
