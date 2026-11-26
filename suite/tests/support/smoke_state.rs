//! Staging for the macro-aware smoke test: the private state directory, the
//! staged suite library, the consumer crates and the nested-build lock.
//!
//! Everything is written through capability roots opened once here, so the
//! harness never hands a caller's path to `std::fs`.

use std::{
    env,
    io,
    path::{Path, PathBuf},
};

use super::{CONSUMER_MANIFEST, OptIn, OrPanic, pinned_toolchain, workspace_root};

/// The private directory this test process owns, and the capabilities it is
/// staged through.
///
/// The process identifier in the name keeps two overlapping runs in one
/// checkout from clearing each other's staged library and consumers.
pub(super) struct State {
    /// Absolute path of the directory, for the processes the harness spawns.
    pub(super) path: PathBuf,
    /// `target/tests`, which holds the directory and the build lock.
    pub(super) parent: whitaker_common::test_support::fixture_fs::Dir,
    /// The directory itself.
    pub(super) root: whitaker_common::test_support::fixture_fs::Dir,
    /// The directory's name inside `parent`.
    pub(super) name: String,
}

/// Open `workspace/target`, creating it when it is missing.
///
/// `target` may be a symlink to another volume, which a `Dir` on the workspace
/// would refuse to cross, so the root is opened on it directly; a missing one is
/// created through the workspace root first.
fn open_target(workspace: &Path) -> io::Result<whitaker_common::test_support::fixture_fs::Dir> {
    let target = workspace.join("target");
    match whitaker_common::test_support::fixture_fs::open_root(&target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            whitaker_common::test_support::fixture_fs::open_root(workspace)?
                .create_dir("target")?;
            whitaker_common::test_support::fixture_fs::open_root(&target)
        }
        other => other,
    }
}

/// Clear stale state and create the directories the test needs.
pub(super) fn prepare_state_dir() -> State {
    let workspace = workspace_root();
    let target = open_target(&workspace).or_panic("failed to open the target directory");
    target
        .create_dir_all("tests")
        .or_panic("failed to create the test state directory");
    let parent = target
        .open_dir("tests")
        .or_panic("failed to open the test state directory");
    let name = format!("macro_aware_smoke-{}", std::process::id());
    match parent.remove_dir_all(&name) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => panic!("failed to clear `{name}`: {error}"),
    }
    parent
        .create_dir_all(format!("{name}/library"))
        .or_panic("failed to create the staging directory");
    let root = parent
        .open_dir(&name)
        .or_panic("failed to open the staging directory");
    State {
        path: workspace_root().join("target/tests").join(&name),
        parent,
        root,
        name,
    }
}

/// Copy the cdylib under the toolchain-qualified name Dylint discovers.
pub(super) fn stage_library(
    source: &Path,
    library_dir: &Path,
    library: &whitaker_common::test_support::fixture_fs::Dir,
) -> PathBuf {
    let Some(path_name) = source.file_name() else {
        panic!("the suite cdylib must have a file name");
    };
    let file_name = path_name.to_string_lossy();
    let suffix = env::consts::DLL_SUFFIX;
    let toolchain = pinned_toolchain();
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    );
    let source_dir = whitaker_common::test_support::fixture_fs::open_root(
        source.parent().unwrap_or_else(|| Path::new(".")),
    )
    .or_panic("failed to open the suite library directory");
    if let Err(error) = source_dir.copy(path_name, library, &target_name) {
        panic!(
            "failed to stage `{}` as `{target_name}`: {error}",
            source.display()
        );
    }
    library_dir.join(target_name)
}

/// Write the fixed consumer crate beneath `consumer` (relative to `state`),
/// configured as `opt_in`.
pub(super) fn write_consumer(state: &State, consumer: &Path, opt_in: OptIn) {
    state
        .root
        .create_dir_all(consumer.join("src"))
        .or_panic("failed to create the consumer directory");
    state
        .root
        .write(consumer.join("Cargo.toml"), CONSUMER_MANIFEST)
        .or_panic("failed to write the consumer manifest");
    state
        .root
        .write(consumer.join("dylint.toml"), opt_in.dylint_config())
        .or_panic("failed to write the consumer lint configuration");
    state
        .root
        .write(consumer.join("src/lib.rs"), opt_in.source())
        .or_panic("failed to write the consumer source");
}

/// Take the cross-process lock that serializes nested suite builds.
///
/// Returns the lock; dropping it releases it.
pub(super) fn lock_nested_builds(
    state: &State,
) -> whitaker_common::test_support::fixture_fs::FileLock {
    whitaker_common::test_support::fixture_fs::lock_exclusive(
        &state.parent,
        "macro_aware_smoke.lock",
    )
    .or_panic("failed to lock the nested-build lock file")
}

#[cfg(all(test, unix))]
mod tests {
    //! Coverage for opening the Cargo target directory.

    use super::*;

    fn temp() -> tempfile::TempDir {
        match tempfile::tempdir() {
            Ok(directory) => directory,
            Err(error) => panic!("temporary directory: {error}"),
        }
    }

    #[test]
    fn a_missing_target_directory_is_created() {
        let workspace = temp();

        let opened = open_target(workspace.path());

        assert!(
            opened.is_ok(),
            "a missing target should be created and opened"
        );
        assert!(
            workspace.path().join("target").is_dir(),
            "the target directory should exist afterwards"
        );
    }

    #[test]
    fn a_target_symlink_to_another_volume_is_followed() {
        let workspace = temp();
        let elsewhere = temp();
        let root = whitaker_common::test_support::fixture_fs::open_root(workspace.path())
            .expect("workspace root");
        root.symlink_contents(elsewhere.path(), "target")
            .expect("target link");

        let opened = open_target(workspace.path());

        assert!(
            opened.is_ok_and(|target| target.create_dir_all("tests").is_ok()),
            "a target behind a symlink should open and accept directories"
        );
        assert!(
            elsewhere.path().join("tests").is_dir(),
            "the directory should be created at the link's target"
        );
    }
}
