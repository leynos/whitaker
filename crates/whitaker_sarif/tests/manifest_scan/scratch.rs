//! A staged temporary directory for tests that must touch the filesystem.
//!
//! Two test modules need real files and neither can use a tracked path.
//! `discovery_tests` stages manifests to exercise the three-way presence
//! answer, and `guard_tests` stages a manifest to drive the on-disk guard
//! through the rejections it must make. Staging under `CARGO_MANIFEST_DIR` is
//! not an option: the workspace contains only the manifests it really has, so a
//! forbidden or unreadable fixture cannot be placed there without corrupting
//! the repository.
//!
//! The helper lives in its own module so both callers share one definition of
//! "a scratch directory". Copies would drift, and a scratch root that stopped
//! being UTF-8-clean or capability-scoped would then fail in one module and not
//! the other.
//!
//! Fixture writes go through a capability-scoped `cap_std` handle for the same
//! reason the rest of this toolkit does: the workspace denies ambient
//! `std::fs`, and a test is not an exemption.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::TempDir;

/// A staged temporary directory: its handle, its UTF-8 root, and a keepalive.
///
/// The `TempDir` field is never read. It exists so the directory is removed
/// when the value drops, which is the whole of its job; the underscore says so.
pub(crate) struct Scratch {
    _directory: TempDir,
    /// The capability-scoped handle fixture files are written through.
    pub(crate) root: Dir,
    root_path: Utf8PathBuf,
}

impl Scratch {
    /// Joins `name` onto the staged root.
    #[must_use]
    pub(crate) fn path(&self, name: &str) -> Utf8PathBuf { self.root_path.join(name) }
}

/// Creates a scratch directory, panicking when the host cannot provide one.
///
/// Panicking is the same outcome `expect()` in a test body would produce, and
/// this is called only from test bodies, where the workspace permits it. That
/// permission is why the staging lives in this module rather than in a
/// `#[fixture]`: the workspace denies `expect_used` and sets
/// `allow-expect-in-tests = true`, which covers a `#[test]` body but not a
/// helper the harness builds before it. A fallible fixture is not available
/// either, because the guard's rejection tests are `#[should_panic]` and the
/// workspace denies `panic_in_result_fn`, so they may not return `Result`.
pub(crate) fn scratch() -> Scratch {
    let directory = match TempDir::new() {
        Ok(directory) => directory,
        Err(error) => panic!("a temporary directory should be creatable: {error}"),
    };
    // Let-else rather than a `match` that yields the path: the workspace also
    // denies `option_if_let_else`, and a two-arm `match` on an `Option` that
    // produces a value is the shape it flags.
    let Some(root_path) = Utf8Path::from_path(directory.path()).map(Utf8Path::to_owned) else {
        panic!("the temporary directory path should be valid UTF-8");
    };
    let root = match Dir::open_ambient_dir(&root_path, ambient_authority()) {
        Ok(root) => root,
        Err(error) => panic!("the temporary directory should open: {error}"),
    };
    Scratch {
        _directory: directory,
        root,
        root_path,
    }
}
