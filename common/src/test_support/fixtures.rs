//! Fixture management utilities for UI test harnesses.
//!
//! These helpers clone fixture source files, stderr expectations, and any
//! supporting assets into a temporary workspace so UI harnesses only need to
//! focus on executing the lint runner.

use std::fs;
use std::io;
use std::path::Path;

const MAX_DIRECTORY_DEPTH: usize = 64;

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
/// The helper rejects any symlink it encounters to avoid accidental
/// traversal outside the fixture tree. Directory recursion is capped at
/// `MAX_DIRECTORY_DEPTH` to prevent runaway traversal when a fixture contains
/// unexpectedly deep nesting.
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
    copy_directory_with_depth(source, destination, MAX_DIRECTORY_DEPTH)
}

fn copy_directory_with_depth(
    source: &Path,
    destination: &Path,
    remaining_depth: usize,
) -> io::Result<()> {
    if remaining_depth == 0 {
        return Err(depth_limit_error(source));
    }

    let metadata = source.symlink_metadata()?;
    ensure_directory(source, &metadata)?;
    ensure_not_symlink(source, metadata.file_type())?;

    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let entry_path = entry.path();
        let file_type = entry_path.symlink_metadata()?.file_type();

        ensure_not_symlink(&entry_path, file_type)?;

        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_directory_with_depth(&entry_path, &target, remaining_depth - 1)?;
        } else {
            fs::copy(&entry_path, target)?;
        }
    }

    Ok(())
}

fn ensure_not_symlink(path: &Path, file_type: fs::FileType) -> io::Result<()> {
    if file_type.is_symlink() {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refusing to follow symlink `{}` while copying fixtures",
                path.display()
            ),
        ))
    } else {
        Ok(())
    }
}

fn ensure_directory(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("`{}` is not a directory", path.display()),
        ))
    }
}

fn depth_limit_error(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "refusing to copy `{}`: directory depth exceeds limit of {} levels",
            path.display(),
            MAX_DIRECTORY_DEPTH
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use tempfile::{TempDir, tempdir};

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

    #[test]
    fn copy_fixture_missing_source_file_errors() {
        let root = tempdir().expect("fixture root");
        let fixture = root.path().join("missing.rs");
        let destination = tempdir().expect("destination root");

        let error = copy_fixture(root.path(), &fixture, destination.path())
            .expect_err("missing source should error");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    fn setup_copy_fixture_test(
        with_stderr: bool,
        with_support: bool,
    ) -> (TempDir, PathBuf, TempDir) {
        let root = tempdir().expect("fixture root");
        let fixture = root.path().join("case.rs");
        fs::write(&fixture, "fn main() {}").expect("fixture file");

        if with_stderr {
            fs::write(root.path().join("case.stderr"), "stderr").expect("stderr file");
        }

        if with_support {
            let support_dir = root.path().join("case");
            fs::create_dir_all(&support_dir).expect("support dir");
            fs::write(support_dir.join("helper.rs"), "fn helper() {}").expect("support helper");
        }

        let destination = tempdir().expect("destination root");
        (root, fixture, destination)
    }

    #[test]
    fn copy_fixture_without_stderr_succeeds() {
        let (root, fixture, destination) = setup_copy_fixture_test(false, false);

        copy_fixture(root.path(), &fixture, destination.path())
            .expect("copy succeeds without stderr");

        assert!(destination.path().join("case.rs").exists());
        assert!(!destination.path().join("case.stderr").exists());
    }

    #[test]
    fn copy_fixture_without_support_directory_succeeds() {
        let (root, fixture, destination) = setup_copy_fixture_test(true, false);

        copy_fixture(root.path(), &fixture, destination.path())
            .expect("copy succeeds without support dir");

        assert!(destination.path().join("case.rs").exists());
        assert!(destination.path().join("case.stderr").exists());
        assert!(!destination.path().join("case").exists());
    }

    #[test]
    fn copy_directory_enforces_depth_limit() {
        let source_root = tempdir().expect("source root");
        let mut current = source_root.path().to_path_buf();
        for level in 0..=MAX_DIRECTORY_DEPTH {
            current = current.join(format!("level_{level}"));
            fs::create_dir_all(&current).expect("nested dir");
        }

        let destination = tempdir().expect("destination root");
        let error = copy_directory(source_root.path(), destination.path())
            .expect_err("deep nesting should error");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains(&MAX_DIRECTORY_DEPTH.to_string()));
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn copy_directory_rejects_symlinks() {
        let source_root = tempdir().expect("source root");
        let file = source_root.path().join("data.txt");
        fs::write(&file, "data").expect("symlink target");
        let link = source_root.path().join("link.txt");
        create_symlink(&file, &link).expect("create symlink");

        let destination = tempdir().expect("destination root");
        let error = copy_directory(source_root.path(), destination.path())
            .expect_err("symlink should error");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("symlink"));
    }

    #[cfg(unix)]
    fn create_symlink(target: &Path, link: &Path) -> io::Result<()> {
        use std::os::unix::fs::symlink;
        symlink(target, link)
    }

    #[cfg(windows)]
    fn create_symlink(target: &Path, link: &Path) -> io::Result<()> {
        use std::os::windows::fs::symlink_file;
        symlink_file(target, link)
    }
}
