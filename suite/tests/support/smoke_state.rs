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

/// Clear stale state and create the directories the test needs.
pub(super) fn prepare_state_dir() -> State {
    let workspace = whitaker_common::test_support::fixture_fs::open_root(workspace_root())
        .or_panic("failed to open the workspace root");
    workspace
        .create_dir_all("target/tests")
        .or_panic("failed to create the test state directory");
    let parent = workspace
        .open_dir("target/tests")
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
