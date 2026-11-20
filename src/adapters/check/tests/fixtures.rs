//! Shared setup for the `check` adapter's tests.
//!
//! Three kinds of fixture live here: platform directories rooted in a
//! temporary directory, the staged library names a real installation writes,
//! and the scripted repair backend. Keeping them together means the tests that
//! pin one route and the tests that pin the other build the same shape, rather
//! than each carrying its own copy that could drift.

use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
};

use camino::Utf8PathBuf;
use tempfile::TempDir;
#[cfg(unix)]
use whitaker_installer::test_utils::SnapshotFixture;
use whitaker_installer::{
    bootstrap::{BOOTSTRAP_SCHEMA, BootstrapLayout, BootstrapMetadata, StoredPath},
    cli::InstallArgs,
    crate_name::CrateName,
    dirs::BaseDirs,
    error::InstallerError,
    resolution::SUITE_CRATE,
    stager::Stager,
};

#[cfg(unix)]
use super::super::repair::SystemSteps;
use super::super::repair::{Readiness, RepairSteps};

/// Stage a toolchain: the only thing a staged library name depends on.
const TOOLCHAIN: &str = "nightly-2026-05-28";

/// Another toolchain, so a second suite on one path is a real second suite.
const OTHER_TOOLCHAIN: &str = "nightly-2026-05-29";

/// Platform directories rooted in one temporary directory.
pub(super) struct TempDirs {
    root: PathBuf,
}

impl TempDirs {
    /// Root every platform directory under `root`.
    fn new(root: &TempDir) -> Self {
        Self {
            root: root.path().to_owned(),
        }
    }

    /// The bootstrap layout a repair resolves for these directories.
    pub(super) fn layout(&self) -> BootstrapLayout {
        BootstrapLayout::from_dirs(self).expect("a temporary root names a bootstrap directory")
    }
}

impl BaseDirs for TempDirs {
    fn home(&self) -> Option<PathBuf> { Some(self.root.join("home")) }

    fn executables(&self) -> Option<PathBuf> { Some(self.root.join("home/.local/bin")) }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.root.join("data/whitaker")) }
}

/// Temporary platform directories, and the temporary directory behind them.
///
/// The directory is returned so the caller keeps it alive: dropping it would
/// remove the record the test just wrote.
pub(super) fn temp_dirs() -> (TempDir, TempDirs) {
    let root = TempDir::new().expect("a temporary directory is available");
    let dirs = TempDirs::new(&root);
    (root, dirs)
}

/// An installation record, carrying `pin` when one is given.
pub(super) fn record(pin: Option<&str>) -> BootstrapMetadata {
    BootstrapMetadata {
        schema_version: BOOTSTRAP_SCHEMA,
        dylint_version: "6.3.0".to_owned(),
        snapshot_commit: "e4608a25".to_owned(),
        toolchain: TOOLCHAIN.to_owned(),
        library_dir: StoredPath::DataRootRelative(Utf8PathBuf::from("whitaker/lints")),
        suite_version: pin.map(str::to_owned),
    }
}

/// Build the staged file name a Whitaker suite library has on this platform.
pub(super) fn suite_filename() -> String { staged_filename(TOOLCHAIN) }

/// Build the same suite's staged file name for another toolchain.
pub(super) fn other_suite_filename() -> String { staged_filename(OTHER_TOOLCHAIN) }

/// Build the staged file name of a lint crate that is not the suite.
///
/// Generated rather than written as a literal so every case reaches the crate
/// comparison on every host: a hard-coded `.so` suffix is not a staged name on
/// macOS or Windows, and the parser would reject the file before comparing.
pub(super) fn other_library_filename() -> String {
    Stager::new(Utf8PathBuf::from("unused"), TOOLCHAIN)
        .staged_filename(&CrateName::from("module_max_lines"))
}

/// Build the staged file name of the suite for `toolchain`.
fn staged_filename(toolchain: &str) -> String {
    Stager::new(Utf8PathBuf::from("unused"), toolchain)
        .staged_filename(&CrateName::from(SUITE_CRATE))
}

/// Create a directory holding one staged suite.
///
/// Both routes approve this shape, so their tests share the setup rather than
/// repeating the same three lines.
pub(super) fn directory_with_suite() -> TempDir {
    let directory = TempDir::new().expect("a temporary directory is available");
    std::fs::write(directory.path().join(suite_filename()), b"")
        .expect("the suite file can be created");
    directory
}

/// Join `directories` into one platform path list.
pub(super) fn path_list(directories: &[&TempDir]) -> OsString {
    std::env::join_paths(directories.iter().map(|dir| dir.path()))
        .expect("temporary paths are representable")
}

/// The installer failure a failing script reports.
fn install_failure() -> InstallerError {
    InstallerError::WorkspaceNotFound {
        reason: "the platform provides no data directory".to_owned(),
    }
}

/// A repair whose readiness answer is fixed and whose install is recorded.
pub(super) struct ScriptedSteps {
    /// The answer the in-lock readiness check gives.
    readiness: Readiness,
    /// How many installs were requested.
    pub(super) installs: usize,
    /// The arguments each install was given, in order.
    pub(super) arguments: Vec<InstallArgs>,
    /// Whether the install reports a failure.
    fails: bool,
    /// A machine marker the install writes to its standard output.
    marker: Option<&'static str>,
}

impl ScriptedSteps {
    /// Script an install that succeeds, given this readiness answer.
    pub(super) fn ready_to_install(readiness: Readiness) -> Self {
        Self {
            readiness,
            installs: 0,
            arguments: Vec::new(),
            fails: false,
            marker: None,
        }
    }

    /// Script an install that fails after writing `marker`.
    pub(super) fn failing(marker: &'static str) -> Self {
        Self {
            marker: Some(marker),
            fails: true,
            ..Self::ready_to_install(Readiness::Repairable)
        }
    }

    /// Make the install write `marker` to its standard output.
    pub(super) fn writing(mut self, marker: &'static str) -> Self {
        self.marker = Some(marker);
        self
    }
}

impl RepairSteps for ScriptedSteps {
    fn readiness(&self, _dirs: &dyn BaseDirs) -> Readiness { self.readiness.clone() }

    fn install(
        &mut self,
        arguments: &InstallArgs,
        stdout: &mut dyn Write,
        _stderr: &mut dyn Write,
    ) -> Result<(), InstallerError> {
        self.installs += 1;
        self.arguments.push(arguments.clone());
        if let Some(marker) = self.marker {
            writeln!(stdout, "{marker}").expect("a buffer accepts the marker");
        }
        if self.fails {
            return Err(install_failure());
        }
        Ok(())
    }
}

/// A repair whose readiness is the production check, whose install is counted.
///
/// Delegating [`RepairSteps::readiness`] to [`SystemSteps`] is the point: which
/// locked preflight failures may be installed over is the policy under test,
/// and a scripted answer would restate that policy rather than exercise it.
///
/// Unix-only because its one consumer provokes a filesystem fault that Windows
/// reports as a missing record instead; compiling it there would leave an
/// unused type, which the workspace's `-D warnings` denies.
#[cfg(unix)]
pub(super) struct RealReadiness {
    /// How many installs were requested.
    pub(super) installs: usize,
}

#[cfg(unix)]
impl RealReadiness {
    /// Start with no installs requested.
    pub(super) const fn new() -> Self { Self { installs: 0 } }
}

#[cfg(unix)]
impl RepairSteps for RealReadiness {
    fn readiness(&self, dirs: &dyn BaseDirs) -> Readiness { SystemSteps.readiness(dirs) }

    fn install(
        &mut self,
        _arguments: &InstallArgs,
        _stdout: &mut dyn Write,
        _stderr: &mut dyn Write,
    ) -> Result<(), InstallerError> {
        self.installs += 1;
        Ok(())
    }
}

/// A complete installation in a temporary data root, plus a stub `cargo`.
///
/// The readiness probe verifies a real snapshot, so this fixture installs the
/// installer's own snapshot bundle rather than faking the check: a value that
/// only a genuine preflight could produce is what a run re-check test needs.
#[cfg(unix)]
pub(super) struct ReadyInstallation {
    /// Owns the data root the installation lives in.
    pub(super) _root: TempDir,
    /// Owns the snapshot fixture's source repository.
    pub(super) _snapshot: SnapshotFixture,
    /// The platform directories that resolve to this installation.
    pub(super) dirs: TempDirs,
    /// The stub `cargo` whose version probe reports the recorded release.
    pub(super) cargo: PathBuf,
}

#[cfg(unix)]
impl ReadyInstallation {
    /// Install a snapshot, a record, and a stub `cargo`.
    ///
    /// # Errors
    ///
    /// Returns whatever the snapshot fixture, the record write, or the stub
    /// write reports, so a caller fails with the real cause rather than a
    /// fixture panic.
    pub(super) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let (root, dirs) = temp_dirs();
        let layout = dirs.layout();
        let snapshot = SnapshotFixture::new()?;
        snapshot.install(&layout.clippy_root())?;
        let data_root = layout.data_root().to_owned();
        // The probe only checks that this directory exists; the suite check
        // the adapter adds is what a test changes.
        let library_dir = data_root.join("whitaker/lints");
        std::fs::create_dir_all(&library_dir)?;
        BootstrapMetadata {
            schema_version: BOOTSTRAP_SCHEMA,
            dylint_version: "6.3.0".to_owned(),
            snapshot_commit: snapshot.commit().to_owned(),
            toolchain: whitaker_installer::test_utils::FIXTURE_TOOLCHAIN.to_owned(),
            library_dir: StoredPath::new(&library_dir, &data_root),
            suite_version: None,
        }
        .write(&layout.metadata_path())?;
        let cargo = data_root.join("cargo-stub").into_std_path_buf();
        std::fs::write(&cargo, stub_cargo())?;
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755))?;
        Ok(Self {
            _root: root,
            _snapshot: snapshot,
            dirs,
            cargo,
        })
    }
}

/// A stream that refuses every write, standing in for a closed descriptor.
///
/// Shared because both repairing and running treat an unwritable stream as a
/// case to pin: one fails the call before touching the installation, the other
/// keeps the status it already has.
pub(super) struct ClosedStream;

impl Write for ClosedStream {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("the stream is closed"))
    }

    fn flush(&mut self) -> io::Result<()> { Err(io::Error::other("the stream is closed")) }
}

/// Render the stub `cargo` the readiness probe accepts.
///
/// The probe runs `cargo dylint --version` and reads the last token of its
/// standard output, so the stub answers exactly that question and fails
/// anything else. A shell script is enough because the probe never runs on a
/// platform without a POSIX shell in this repository's test matrix.
#[cfg(unix)]
fn stub_cargo() -> String {
    concat!(
        "#!/usr/bin/env bash\n",
        "if [[ \"$1\" == dylint && \"$2\" == --version ]]; then\n",
        "    echo \"cargo-dylint 6.3.0\"\n",
        "    exit 0\n",
        "fi\n",
        "exit 1\n",
    )
    .to_owned()
}
