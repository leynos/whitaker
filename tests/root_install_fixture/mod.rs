//! Fixture for the root `whitaker install` end-to-end test.
//!
//! The fixture keeps the test target's own file focused on what a successful
//! root install must produce. It supplies the isolated data, bin, and home
//! roots, a fixture Clippy snapshot, the synthetic staging seam the installer's
//! behaviour tests use, and the toolchain probe that decides whether the real
//! install can run on this host. A stub `cargo` and a forwarding command let
//! the test follow the record the install wrote out to a lint run started
//! outside a checkout.

mod host_home;
#[cfg(unix)]
mod stub_cargo;

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[cfg(unix)]
pub use stub_cargo::{STUB_EXIT_CODE, StubCargo};
use tempfile::TempDir;
use whitaker_installer::{
    builder::{library_extension, library_prefix},
    dirs::{BaseDirs, SystemBaseDirs},
    test_support::{TEST_DATA_HOME_ENV, TEST_EXECUTABLE_HOME_ENV, TEST_STAGE_SUITE_ENV},
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
/// compatibility scripts. The bin root starts with a `whitaker` executable of
/// its own, so the test also proves a root install leaves an existing binary
/// alone rather than replacing it with a wrapper.
///
/// Both roots are named through the installer's debug-only overrides rather
/// than `XDG_DATA_HOME` and `XDG_BIN_HOME`, because Windows resolves each from
/// a platform API that ignores those variables. The overrides keep the
/// installation record and the executable directory inside the fixture's
/// temporary tree on every platform, so the test cannot alter a record or a
/// binary a user already has, and the bin-root assertions inspect the directory
/// the child actually resolved. `XDG_DATA_HOME`, `XDG_BIN_HOME`, and `HOME` are
/// still set for the Unix paths that honour them.
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
    ///
    /// `HOME` is redirected, but `CARGO_HOME` and `RUSTUP_HOME` are not:
    /// rustup would derive both from the temporary `HOME`, find no rustup
    /// installation there, and abort the install's toolchain step with
    /// `rustup is not installed at '<home>/.cargo'`. Passing the host's
    /// directories through keeps the toolchain work the install does
    /// consistent with [`is_toolchain_installed`], which probed the parent
    /// environment before the test decided to run.
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
            .env(TEST_EXECUTABLE_HOME_ENV, self.bin_home.path())
            .env("XDG_DATA_HOME", self.data_home.path())
            .env("XDG_BIN_HOME", self.bin_home.path())
            .env("HOME", self.home.path())
            .env_remove("DYLINT_LIBRARY_PATH");
        for (variable, conventional) in [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")] {
            if let Some(host_home) = host_home::host_home(variable, conventional) {
                command.env(variable, host_home);
            }
        }
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

    /// Create a directory outside the checkout, as a user's own project is.
    ///
    /// # Errors
    ///
    /// Returns a description of the failed directory creation.
    #[cfg(unix)]
    pub fn external_project() -> Result<TempDir, String> { temporary_directory("external project") }

    /// Build the root CLI command that forwards a lint run from `working_dir`.
    ///
    /// The command resolves its library path from the record the fixture's own
    /// install wrote, so the assertion that follows tests the record an install
    /// produces rather than one a test wrote by hand. The stub `cargo` keeps
    /// the real Dylint out of the run and records what it would have received.
    ///
    /// # Errors
    ///
    /// Returns a description of a `PATH` that cannot be joined.
    #[cfg(unix)]
    pub fn forward_command(&self, working_dir: &Path, stub: &StubCargo) -> Result<Command, String> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker"));
        command
            .arg("--all")
            .current_dir(working_dir)
            .env(TEST_DATA_HOME_ENV, self.data_root())
            .env(TEST_EXECUTABLE_HOME_ENV, self.bin_home.path())
            .env("XDG_DATA_HOME", self.data_home.path())
            .env("XDG_BIN_HOME", self.bin_home.path())
            .env("HOME", self.home.path())
            .env("PATH", stub.path_with_stub()?)
            .env("STUB_LOG", stub.log_path())
            .env("STUB_EXIT", STUB_EXIT_CODE.to_string())
            .env_remove("DYLINT_LIBRARY_PATH")
            .env_remove("DYLINT_CLIPPY_REPO")
            .env_remove("DYLINT_DRIVER_PATH");
        Ok(command)
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

/// Report whether the child will honour the fixture's directory overrides.
///
/// Both overrides are compiled out of release builds, so a release-profile test
/// binary cannot keep the child inside the fixture's tree: the child would use
/// the developer's real data and executable directories. Refusing to spawn it
/// is the only safe response, and CI fails rather than skipping, because a CI
/// run where the only end-to-end proof of a root install stops running would
/// otherwise pass unnoticed.
pub fn child_isolation_is_available() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }
    report_unisolated_run();
    false
}

/// Fail in CI, or report a skip, when the build compiles the overrides out.
fn report_unisolated_run() {
    assert!(
        std::env::var_os("CI").is_none(),
        "the root install end-to-end test needs an unoptimized, debug-assertion build"
    );
    tracing::info!("skipping: this build cannot isolate the child's data and bin roots");
}
