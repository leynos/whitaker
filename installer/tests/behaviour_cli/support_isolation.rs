//! Isolate a non-dry-run installer child from the host's data root.
//!
//! Installation provisions the Clippy snapshot and records both the bootstrap
//! and the staged lint paths, so a child that ran against the host's own data
//! root would leave real state behind. These helpers own the fixture-owned
//! directories, the environment that points the child at them, and the
//! preflight that refuses a child which could not honour that environment.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::TempDir;
use whitaker_installer::{
    dirs::{BaseDirs, SystemBaseDirs},
    test_support::TEST_DATA_HOME_ENV,
    test_utils::SnapshotFixture,
};

use super::CliWorld;

// Reject a child that cannot honour the data-root override, before it runs.
//
// The override is compiled out of release builds, so a release-built child
// would ignore it and write its bootstrap and installation records to the real
// data directory. `CARGO_BIN_EXE_*` is built with the same profile as this
// test, so the test's own build profile is the honest preflight. A
// compile-time rejection is the only form that cannot be optimized away or
// read as a constant assertion: this test binary simply does not build in a
// release profile.
#[cfg(not(debug_assertions))]
compile_error!(
    "the installer only honours WHITAKER_INSTALLER_TEST_DATA_HOME in debug builds, so a \
     release-built child would write to the real data directory; run these tests in a debug \
     profile"
);

/// The fixture-owned directories a non-dry-run child installs into.
///
/// Both are temporary directories, so they must outlive the child process and
/// are kept in the world rather than dropped when the command returns.
pub(super) struct InstallIsolation {
    /// The small real Clippy snapshot the child provisions from.
    _snapshot: SnapshotFixture,
    /// The data home the child resolves its data and bootstrap roots under.
    data_home: TempDir,
    /// The host's own installation record, as it stood before the child ran.
    real_record: Option<RealRecord>,
}

impl InstallIsolation {
    /// The data root the child was told to use.
    pub(super) fn data_root(&self) -> PathBuf { self.data_home.path().join("whitaker") }

    /// The directories the child resolved, for predicting where it wrote.
    ///
    /// An assertion that predicts a path the child created must resolve it the
    /// same way the child did. The platform directories would name the host's
    /// real data root, which the child never touched.
    pub(super) const fn dirs(&self) -> IsolatedDirs<'_> { IsolatedDirs(self) }

    /// Assert the child left the host's own installation record alone.
    ///
    /// The fixture record assertion proves the child wrote *somewhere* inside
    /// the fixture. This proves the stronger property the isolation exists
    /// for: the host's real record is byte-for-byte what it was before.
    pub(super) fn assert_real_record_unchanged(&self) {
        if let Some(record) = &self.real_record {
            record.assert_unchanged();
        }
    }
}

/// A [`BaseDirs`] view of the fixture-owned data root.
pub(super) struct IsolatedDirs<'a>(&'a InstallIsolation);

impl BaseDirs for IsolatedDirs<'_> {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.0.data_root()) }
}

/// The host's installation record, captured so a run can prove it is untouched.
struct RealRecord {
    /// Where the host records its staged lint paths.
    path: PathBuf,
    /// The record's bytes before the child ran, or `None` if it was absent.
    contents: Option<Vec<u8>>,
}

impl RealRecord {
    /// Capture the host's record, if the platform can name its data directory.
    fn capture() -> Option<Self> {
        let dirs = SystemBaseDirs::new()?;
        let path = dirs.whitaker_data()?.join("installed_lint_paths.json");
        let contents = read_record(&path);
        Some(Self { path, contents })
    }

    /// Fail if the record's bytes differ from the captured ones.
    fn assert_unchanged(&self) {
        let after = read_record(&self.path);
        assert!(
            after == self.contents,
            "the child must not write the host's real installation record at {}",
            self.path.display()
        );
    }
}

/// Read the record, distinguishing absence from an unreadable record.
///
/// `NotFound` is genuine absence: the host has never recorded an installation.
/// Any other error means the comparison cannot be made, so it fails here
/// rather than being read as absence. Treating both reads as `None` would let
/// the assertion pass without having checked the record at all.
fn read_record(path: &Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!(
            "the host's installation record at {} should be readable: {error}",
            path.display()
        ),
    }
}

/// Give a non-dry-run install a small real snapshot and a data root of its own.
///
/// Installation provisions the Clippy snapshot before anything else and
/// records both the bootstrap and the staged lint paths in the data root, so
/// every non-dry-run child must be isolated from the host. `XDG_DATA_HOME`
/// alone is not enough: Windows and macOS resolve the data directory from a
/// platform API that ignores it, so the debug-only override is set as well.
pub(super) fn isolate_install(command: &mut Command, cli_world: &CliWorld) -> InstallIsolation {
    let Ok(snapshot) = SnapshotFixture::new() else {
        panic!("the fixture Clippy snapshot should build");
    };
    let Ok(data_home) = TempDir::new() else {
        panic!("an isolated data home should be created");
    };
    snapshot.configure(command);
    command.env("XDG_DATA_HOME", data_home.path());
    let isolation = InstallIsolation {
        _snapshot: snapshot,
        data_home,
        real_record: RealRecord::capture(),
    };
    command.env(TEST_DATA_HOME_ENV, isolation.data_root());
    if cli_world.block_record_write.get() {
        block_record_write(&isolation.data_root());
    }
    isolation
}

/// Plant a directory where the installation record belongs.
///
/// Every write to the record then fails, which is the field case of an
/// unwritable data directory.
fn block_record_write(data_root: &Path) {
    let record = data_root.join("installed_lint_paths.json");
    if let Err(error) = std::fs::create_dir_all(&record) {
        panic!("the blocked record path should be created: {error}");
    }
}
