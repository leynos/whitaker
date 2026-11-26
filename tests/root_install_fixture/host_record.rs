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
    let read = read_following_link(&data, "installed_lint_paths.json");
    match read {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{} should be readable: {error}", path.display())),
    }
}

/// How many symlinks one read follows before it gives up.
const MAX_LINK_HOPS: usize = 40;

/// Read `name` from the directory `data`, following symlinks, including chained
/// ones, that lead outside it.
///
/// The host's record may be a symlink to a relocated data directory, and a
/// `Dir` refuses to follow one, so each link's target is read through a root of
/// its own, until a file is reached.
fn read_following_link(data: &Path, name: &str) -> std::io::Result<Vec<u8>> {
    let mut directory = data.to_path_buf();
    let mut entry = std::path::PathBuf::from(name);
    // Following `MAX_LINK_HOPS` links takes one more read to reach the file.
    for _ in 0..=MAX_LINK_HOPS {
        let current = whitaker_common::test_support::fixture_fs::open_root(&directory)?;
        let Ok(link) = current.read_link_contents(&entry) else {
            return current.read(&entry);
        };
        let resolved = directory.join(&link);
        let (Some(parent), Some(file)) = (resolved.parent(), resolved.file_name()) else {
            return current.read(&entry);
        };
        entry = std::path::PathBuf::from(file);
        directory = parent.to_path_buf();
    }
    Err(std::io::Error::other("too many levels of symbolic links"))
}

#[cfg(all(test, unix))]
mod tests {
    //! Coverage for reading a record behind chained symlinks.

    use super::*;

    fn temp() -> tempfile::TempDir {
        match tempfile::tempdir() {
            Ok(directory) => directory,
            Err(error) => panic!("temporary directory: {error}"),
        }
    }

    #[test]
    fn a_record_behind_two_links_outside_its_directory_is_read() {
        let data = temp();
        let middle = temp();
        let target = temp();
        let data_root =
            whitaker_common::test_support::fixture_fs::open_root(data.path()).expect("data root");
        let middle_root = whitaker_common::test_support::fixture_fs::open_root(middle.path())
            .expect("middle root");
        let target_root = whitaker_common::test_support::fixture_fs::open_root(target.path())
            .expect("target root");
        target_root
            .write("record.json", "payload")
            .expect("write record");
        middle_root
            .symlink_contents(target.path().join("record.json"), "second")
            .expect("second link");
        data_root
            .symlink_contents(middle.path().join("second"), "record.json")
            .expect("first link");

        let read = read_following_link(data.path(), "record.json");

        assert_eq!(
            read.ok().as_deref(),
            Some(b"payload".as_slice()),
            "both links should be followed to the record"
        );
    }

    /// Link `record-0` through `record-{count}` in one directory, ending at a file.
    fn chain(directory: &Path, count: usize) -> std::io::Result<()> {
        let root = whitaker_common::test_support::fixture_fs::open_root(directory)?;
        root.write(format!("record-{count}"), "payload")?;
        for index in (0..count).rev() {
            root.symlink_contents(
                directory.join(format!("record-{}", index + 1)),
                format!("record-{index}"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn a_chain_of_exactly_the_limit_is_followed() {
        let data = temp();
        chain(data.path(), MAX_LINK_HOPS).expect("build the chain");

        let read = read_following_link(data.path(), "record-0");

        assert_eq!(
            read.ok().as_deref(),
            Some(b"payload".as_slice()),
            "a chain of {MAX_LINK_HOPS} links should be followed"
        );
    }

    #[test]
    fn a_chain_past_the_limit_and_a_cycle_are_errors() {
        let longer = temp();
        chain(longer.path(), MAX_LINK_HOPS + 1).expect("build the long chain");
        let cyclic = temp();
        let root = whitaker_common::test_support::fixture_fs::open_root(cyclic.path())
            .expect("cycle root");
        root.symlink_contents(cyclic.path().join("b"), "a")
            .expect("a");
        root.symlink_contents(cyclic.path().join("a"), "b")
            .expect("b");

        assert!(
            read_following_link(longer.path(), "record-0").is_err(),
            "a chain longer than the limit should fail"
        );
        assert!(
            read_following_link(cyclic.path(), "a").is_err(),
            "a cycle should fail"
        );
    }

    #[test]
    fn a_missing_record_is_not_found() {
        let data = temp();
        let read = read_following_link(data.path(), "absent.json");

        assert_eq!(
            read.err().map(|error| error.kind()),
            Some(std::io::ErrorKind::NotFound),
            "a missing record should be NotFound"
        );
    }
}
