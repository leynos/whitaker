//! The host's installation record, read without altering it.

use std::path::Path;

use whitaker_installer::dirs::{BaseDirs as _, SystemBaseDirs};

/// Read the installation record of the real platform data directory.
///
/// Comparing this before and after the install shows that isolating the child
/// left any record a developer already has untouched. A missing record is
/// `None` rather than an error, because that is the normal state on a machine
/// that has never installed Whitaker.
///
/// # Errors
///
/// Returns a description of a data directory that cannot be resolved or a
/// record that exists but cannot be read.
pub fn real_record_contents() -> Result<Option<Vec<u8>>, String> {
    let dirs = SystemBaseDirs::new()
        .ok_or_else(|| "the platform directories should resolve".to_owned())?;
    let data = dirs
        .whitaker_data()
        .ok_or_else(|| "the platform data directory should be available".to_owned())?;
    let path = data.join("installed_lint_paths.json");
    let root = match whitaker_common::test_support::fixture_fs::open_root(&data) {
        Ok(root) => root,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{} should be readable: {error}", data.display())),
    };
    let read = read_following_link(&root, &data, "installed_lint_paths.json");
    match read {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{} should be readable: {error}", path.display())),
    }
}

/// Read `name` from `root` (the directory `data`), following a symlink that
/// leads outside it.
///
/// The host's record may be a symlink to a relocated data directory, and a
/// `Dir` refuses to follow one, so a link's target is read through a root of
/// its own.
fn read_following_link(
    root: &whitaker_common::test_support::fixture_fs::Dir,
    data: &Path,
    name: &str,
) -> std::io::Result<Vec<u8>> {
    let Ok(link) = root.read_link_contents(name) else {
        return root.read(name);
    };
    let resolved = data.join(link);
    match resolved.parent().zip(resolved.file_name()) {
        Some((parent, file)) => whitaker_common::test_support::fixture_fs::open_root(parent)
            .and_then(|target_root| target_root.read(file)),
        None => root.read(name),
    }
}
