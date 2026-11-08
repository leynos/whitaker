//! Reproduce the issue #357 non-finding with two shallow nested regions.
//!
//! Preserve the control-flow shape of rstest-bdd PR #657 at 482b0f0d:
//! a prefix-validation match arm, a common-prefix while-let loop, and a
//! let-else conversion. Standard paths intentionally match the source case.

use std::path::{Component, Path};

/// Express a path relative to a manifest without accessing the filesystem.
///
/// # Errors
///
/// Return an error for incompatible prefixes or non-UTF-8 components.
fn relative_to_manifest(path: &Path, manifest: &Path) -> Result<String, ()> {
    match (path.components().next(), manifest.components().next()) {
        (Some(Component::Prefix(a)), Some(Component::Prefix(b))) => {
            if a.kind() != b.kind() || a.as_os_str() != b.as_os_str() {
                return Err(());
            }
        }
        (Some(Component::Prefix(_)), None) | (None, Some(Component::Prefix(_))) => {
            return Err(());
        }
        _ => {}
    }

    let target: Vec<Component<'_>> = path.components().collect();
    let base: Vec<Component<'_>> = manifest.components().collect();
    let mut common = 0;
    while let (Some(a), Some(b)) = (target.get(common), base.get(common)) {
        if a.as_os_str() != b.as_os_str() {
            break;
        }
        common += 1;
    }

    let mut parts: Vec<String> = Vec::new();
    for _ in common..base.len() {
        parts.push("..".to_owned());
    }
    for component in target.iter().skip(common) {
        let Some(text) = component.as_os_str().to_str() else {
            return Err(());
        };
        parts.push(text.to_owned());
    }
    Ok(parts.join("/"))
}

/// Exercise the fixture without adding another conditional region.
fn main() { let _ = relative_to_manifest(Path::new("/a/b"), Path::new("/a/c")); }
