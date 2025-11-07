//! Fixture management utilities for UI test harnesses.
//!
//! These helpers clone fixture source files, stderr expectations, and any
//! supporting assets into a temporary workspace so UI harnesses only need to
//! focus on executing the lint runner.

use fs_extra::dir::{CopyOptions, copy as copy_dir};
use std::fs;
use std::io;
use std::path::Path;

/// Copies a UI fixture and its optional support directory into `destination`.
///
/// The helper mirrors the `.rs` source file, the `.stderr` expectation (when
/// present), and any sibling directory named after the fixture stem. This
/// mirrors the layout expected by `dylint_testing::ui::Test::src_base`.
///
/// # Examples
///
/// ```
/// use common::test_support::fixtures::copy_fixture;
/// use std::fs;
/// use std::path::PathBuf;
/// use tempfile::tempdir;
///
/// # fn demo() -> std::io::Result<()> {
/// let fixtures = tempdir()?;
/// let fixture = fixtures.path().join("case.rs");
/// fs::write(&fixture, "fn main() {}")?;
/// let destination = tempdir()?;
/// copy_fixture(fixtures.path(), &fixture, destination.path())?;
/// assert!(destination.path().join("case.rs").exists());
/// # Ok(())
/// # }
/// ```
pub fn copy_fixture(fixture_root: &Path, source: &Path, destination_root: &Path) -> io::Result<()> {
    let file_name = source
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "fixture missing file name"))?;
    let destination = destination_root.join(file_name);
    fs::copy(source, &destination)?;

    let stderr_path = source.with_extension("stderr");
    if stderr_path.exists() {
        let stderr_name = stderr_path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "stderr missing name"))?;
        fs::copy(&stderr_path, destination_root.join(stderr_name))?;
    }

    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "fixture missing name"))?;
    let support_dir = fixture_root.join(stem);
    if support_dir.exists() {
        copy_directory(&support_dir, &destination_root.join(stem))?;
    }

    Ok(())
}

/// Recursively copies `source` into `destination`, overwriting existing files.
///
/// # Examples
///
/// ```
/// use common::test_support::fixtures::copy_directory;
/// use std::fs;
/// use tempfile::tempdir;
///
/// # fn demo() -> std::io::Result<()> {
/// let source = tempdir()?;
/// fs::write(source.path().join("data.txt"), "contents")?;
/// let destination = tempdir()?;
/// copy_directory(source.path(), destination.path())?;
/// assert!(destination.path().join("data.txt").exists());
/// # Ok(())
/// # }
/// ```
pub fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    let mut options = CopyOptions::new();
    options.copy_inside = true;
    options.overwrite = true;
    copy_dir(source, destination, &options)
        .map(|_| ())
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn copy_fixture_clones_support_assets() {
        let root = tempdir().expect("fixture root");
        let fixture = root.path().join("case.rs");
        fs::write(&fixture, "fn main() {}").expect("fixture file");
        fs::write(root.path().join("case.stderr"), "stderr").expect("stderr file");

        let support_dir = root.path().join("case");
        fs::create_dir_all(&support_dir).expect("support dir");
        fs::write(support_dir.join("helper.txt"), "data").expect("support asset");

        let destination = tempdir().expect("destination root");
        copy_fixture(root.path(), &fixture, destination.path()).expect("copy succeeds");

        assert!(destination.path().join("case.rs").exists());
        assert!(destination.path().join("case.stderr").exists());
        assert!(destination.path().join("case").join("helper.txt").exists());
    }

    #[test]
    fn copy_directory_preserves_nested_files() {
        let source_root = tempdir().expect("source root");
        let nested = source_root.path().join("nested");
        fs::create_dir_all(&nested).expect("nested dir");
        fs::write(nested.join("file.txt"), "data").expect("nested file");
        let destination_root = tempdir().expect("destination root");
        let destination = destination_root.path().join("copy");

        copy_directory(source_root.path(), &destination).expect("copy succeeds");

        assert!(destination.join("nested").join("file.txt").exists());
    }
}
