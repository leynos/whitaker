//! Fixture for the root `whitaker install` end-to-end test.
//!
//! The fixture keeps the test target's own file focused on what a successful
//! root install must produce. It supplies the isolated data, bin, and home
//! roots, a fixture Clippy snapshot, the synthetic staging seam the installer's
//! behaviour tests use, and the toolchain probe that decides whether the real
//! install can run on this host.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;
use whitaker_installer::{
    builder::{library_extension, library_prefix},
    dirs::{BaseDirs, SystemBaseDirs},
    test_support::{TEST_DATA_HOME_ENV, TEST_STAGE_SUITE_ENV},
    test_utils::SnapshotFixture,
};

/// Read the toolchain channel pinned by `rust-toolchain.toml`.
///
/// The fixture Clippy snapshot names the same channel, so the test installs
/// into a toolchain the manifest and the snapshot agree on.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read, parsed, or does not
/// declare a channel.
pub fn pinned_toolchain_channel() -> Result<String, String> {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("rust-toolchain.toml");
    let contents = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{} should be readable: {error}", manifest_path.display()))?;
    let manifest: toml::Value = toml::from_str(&contents)
        .map_err(|error| format!("{} should parse as TOML: {error}", manifest_path.display()))?;
    manifest
        .get("toolchain")
        .and_then(|toolchain| toolchain.get("channel"))
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            format!(
                "{} should declare toolchain.channel",
                manifest_path.display()
            )
        })
}

/// Report whether `rustup` can run the pinned channel.
///
/// Installation verifies the toolchain before staging anything, so a host
/// without the pinned nightly cannot exercise this path. Such a host skips the
/// test rather than failing it, matching the installer's behaviour tests.
pub fn is_toolchain_installed(channel: &str) -> bool {
    Command::new("rustup")
        .args(["run", channel, "rustc", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The staged suite library the synthetic staging step writes.
///
/// Mirrors `Stager::staged_filename`, whose naming the test would otherwise
/// only rediscover by reading the staging directory.
pub fn expected_library_path(target_dir: &Path, channel: &str) -> PathBuf {
    let file_name = format!(
        "{}whitaker_suite@{channel}{}",
        library_prefix(),
        library_extension()
    );
    target_dir.join(channel).join("release").join(file_name)
}

/// Create a temporary directory, naming it in the failure message.
///
/// # Errors
///
/// Returns a description of the failed directory creation.
fn temporary_directory(label: &str) -> Result<TempDir, String> {
    TempDir::new().map_err(|error| format!("temporary {label} should be created: {error}"))
}

/// A [`BaseDirs`] whose data root is a temporary directory.
pub struct FixtureDirs {
    data: PathBuf,
}

impl BaseDirs for FixtureDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data.clone()) }
}

/// The name the fixture's pre-existing executable occupies in the bin root.
pub const SEEDED_BINARY_NAME: &str = "whitaker";

/// A recognizable payload for the executable a user may already have installed.
pub const SEEDED_BINARY_BYTES: &[u8] = b"#!/bin/sh\nprintf 'pre-existing whitaker'\n";

/// The isolated roots and Clippy snapshot one install run writes into.
///
/// `--skip-wrapper` is deliberately not used: leaving wrapper generation in
/// the run is what lets the test observe that a root install writes no
/// compatibility scripts, while `XDG_BIN_HOME` and `HOME` keep the check away
/// from the real user directories on the platforms that honour them. The bin
/// root starts with a `whitaker` executable of its own, so the test also proves
/// a root install leaves an existing binary alone rather than replacing it with
/// a wrapper.
///
/// The data root is named through the installer's debug-only override rather
/// than `XDG_DATA_HOME`, because Windows resolves its data directory from a
/// platform API that ignores that variable. The override keeps the
/// installation record inside the fixture's temporary tree on every platform,
/// so the test cannot alter a record a user already has.
///
/// The bin root has the same limit on Windows, where the home directory comes
/// from the profile known folder and ignores `HOME`. The install therefore
/// never looks at the fixture's bin directory there, so the bin-root assertions
/// prove the no-wrapper policy on Unix only; on Windows they still hold, but
/// only because the install wrote nothing into the fixture directory. Seeding
/// the real profile directory instead would risk overwriting a user's own
/// `whitaker` executable, so the fixture does not do that. Nothing writes to
/// the real bin root either, because a root install generates no scripts.
pub struct InstallFixture {
    channel: String,
    snapshot: SnapshotFixture,
    target_dir: TempDir,
    data_home: TempDir,
    bin_home: TempDir,
    home: TempDir,
}

impl InstallFixture {
    /// Build the isolated roots and the Clippy snapshot they install.
    ///
    /// # Errors
    ///
    /// Returns a description of the first fixture step that failed.
    pub fn new() -> Result<Self, String> {
        let fixture = Self {
            channel: pinned_toolchain_channel()?,
            snapshot: SnapshotFixture::new()
                .map_err(|error| format!("fixture Clippy snapshot should build: {error}"))?,
            target_dir: temporary_directory("install target")?,
            data_home: temporary_directory("data home")?,
            bin_home: temporary_directory("bin home")?,
            home: temporary_directory("home")?,
        };
        fixture.seed_existing_binary()?;
        Ok(fixture)
    }

    /// Place an executable where a wrapper script would otherwise be written.
    ///
    /// The root install writes no scripts, so this file stands in for the
    /// binary the user installed themselves; the test then proves the install
    /// neither replaced nor removed it.
    ///
    /// # Errors
    ///
    /// Returns a description of the failed write or permission change.
    fn seed_existing_binary(&self) -> Result<(), String> {
        let path = self.seeded_binary();
        std::fs::write(&path, SEEDED_BINARY_BYTES)
            .map_err(|error| format!("{} should be written: {error}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| format!("{} should be executable: {error}", path.display()))?;
        }
        Ok(())
    }

    /// The path of the executable the fixture seeds in the bin root.
    pub fn seeded_binary(&self) -> PathBuf { self.bin_home.path().join(SEEDED_BINARY_NAME) }

    /// The bin root the install may write a `whitaker` executable into.
    pub fn bin_root(&self) -> &Path { self.bin_home.path() }

    /// The target directory the install stages the suite under.
    pub fn target_root(&self) -> &Path { self.target_dir.path() }

    /// The pinned toolchain channel the fixture installs for.
    pub fn channel(&self) -> &str { &self.channel }

    /// The directory the synthetic staging step writes the suite into.
    pub fn staging_directory(&self) -> PathBuf {
        self.target_dir.path().join(&self.channel).join("release")
    }

    /// The data directories the root binary resolves for this fixture.
    pub fn dirs(&self) -> FixtureDirs {
        FixtureDirs {
            data: self.data_root(),
        }
    }

    /// The data root the child process resolves for this fixture.
    ///
    /// The command names it through the installer's debug-only override, so
    /// the record lands in the fixture's temporary tree on every platform. The
    /// assertion that reads this root therefore also proves the child honoured
    /// the override rather than falling back to the user's real data directory.
    #[must_use]
    pub fn data_root(&self) -> PathBuf { self.data_home.path().join("whitaker") }

    /// Build the `whitaker install` command for an offline fixture install.
    ///
    /// `--build-only` keeps the prebuilt download out of the run and
    /// `--skip-deps` keeps the host's Dylint tools out of it, so the only work
    /// left is the staged suite, the record, and the root finalization.
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker"));
        command
            .arg("install")
            .arg("--build-only")
            .arg("--skip-deps")
            .arg("--toolchain")
            .arg(&self.channel)
            .arg("--target-dir")
            .arg(self.target_dir.path())
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env(TEST_STAGE_SUITE_ENV, "1")
            .env(TEST_DATA_HOME_ENV, self.data_root())
            .env("XDG_DATA_HOME", self.data_home.path())
            .env("XDG_BIN_HOME", self.bin_home.path())
            .env("HOME", self.home.path())
            .env_remove("DYLINT_LIBRARY_PATH");
        self.snapshot.configure(&mut command);
        command
    }

    /// Run the fixture install to completion.
    ///
    /// # Errors
    ///
    /// Returns a description of the failed process start.
    pub fn run(&self) -> Result<Output, String> {
        self.command()
            .output()
            .map_err(|error| format!("the root install process should start: {error}"))
    }
}

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
    match std::fs::read(&path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{} should be readable: {error}", path.display())),
    }
}

/// Report whether the pinned toolchain is installed, failing in CI when it is
/// not.
///
/// A developer without the pinned toolchain gets a skip instead of a spurious
/// failure, but a silent skip in CI would let the only end-to-end proof of a
/// root install stop running unnoticed, so CI treats a missing toolchain as a
/// failure.
pub fn toolchain_is_available(fixture: &InstallFixture) -> bool {
    let channel = fixture.channel();
    if is_toolchain_installed(channel) {
        return true;
    }
    report_missing_toolchain(channel);
    false
}

/// Fail in CI, or report a skip, when the pinned toolchain is absent.
fn report_missing_toolchain(channel: &str) {
    assert!(
        std::env::var_os("CI").is_none(),
        "the pinned rustup toolchain {channel} must be installed in CI"
    );
    tracing::info!(%channel, "skipping: the pinned rustup toolchain is not installed");
}
