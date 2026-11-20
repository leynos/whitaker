//! The generated `whitaker-ls` wrapper drives the installed bootstrap end to
//! end, from the script through the real installer to a strict fake `cargo`.
//!
//! Linux only: the bootstrap takes its data root from `XDG_DATA_HOME`, which
//! other platforms do not use.
#![cfg(target_os = "linux")]

// The shared fixture exposes helpers this target does not need; the other
// bootstrap targets use them.
#[expect(
    dead_code,
    reason = "the shared bootstrap fixture is larger than this target's needs"
)]
mod bootstrap_support;
mod whitaker_ls_support;

use std::{
    error::Error,
    path::PathBuf,
    process::{Command, Output},
};

use bootstrap_support::{Installation, TOOLCHAIN};
use tempfile::TempDir;
use whitaker_installer::{dirs::BaseDirs, wrapper::generate_wrapper_scripts};

/// Directory resolver that confines the generated wrappers to a temporary bin
/// directory.
struct TempBaseDirs {
    bin: PathBuf,
}

impl BaseDirs for TempBaseDirs {
    /// Return no home directory; wrapper generation does not need one.
    fn home(&self) -> Option<PathBuf> { None }

    /// Return the temporary bin directory.
    fn executables(&self) -> Option<PathBuf> { Some(self.bin.clone()) }

    /// Return no data directory; wrapper generation does not need one.
    fn whitaker_data(&self) -> Option<PathBuf> { None }
}

/// An installation whose generated lister runs against a strict fake `cargo`.
struct LsFixture {
    installation: Installation,
    /// Keeps the wrapper directory alive for the fixture's lifetime.
    _bin: TempDir,
    wrapper: PathBuf,
}

impl LsFixture {
    /// Install the fixture and generate the wrappers beside a copy of the real
    /// installer, so the wrapper's adjacent lookup selects it.
    ///
    /// # Errors
    ///
    /// Returns an error if the installation, wrapper generation, or executable
    /// copy fails.
    fn new() -> Result<Self, Box<dyn Error>> {
        let installation = Installation::new()?;
        whitaker_ls_support::use_strict_cargo(&installation)?;
        let bin = TempDir::new()?;
        let dirs = TempBaseDirs {
            bin: bin.path().to_path_buf(),
        };
        let result = generate_wrapper_scripts(&dirs)?;
        std::fs::copy(
            env!("CARGO_BIN_EXE_whitaker-installer"),
            bin.path().join("whitaker-installer"),
        )?;
        Ok(Self {
            installation,
            _bin: bin,
            wrapper: result.whitaker_ls_path,
        })
    }

    /// Build a lister command with the fixture's isolated environment.
    ///
    /// The fake `cargo` comes first on `PATH`; the inherited entries after it
    /// supply the `bash` and `awk` the wrapper itself needs.
    fn command(&self) -> Command {
        let base = self.installation.base();
        let path = format!(
            "{}:{}",
            base.join("bin"),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut command = Command::new(&self.wrapper);
        command
            .env("XDG_DATA_HOME", &self.installation.data_root)
            .env("HOME", base.join("home"))
            .env("PATH", path)
            .env("STUB_LOG", base.join("stub"))
            .env("STUB_TOOLCHAIN", TOOLCHAIN);
        command
    }

    /// Run the generated lister and wait for it.
    ///
    /// # Errors
    ///
    /// Returns an error if the wrapper cannot be launched or awaited.
    fn run(&self) -> Result<Output, Box<dyn Error>> { Ok(self.command().output()?) }
}

/// Assert a run succeeded, reporting both streams when it did not.
fn assert_succeeded(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The arguments the strict stub recorded for its `dylint list` invocation.
fn recorded_arguments(installation: &Installation) -> Vec<String> {
    installation
        .recorded()
        .into_iter()
        .filter_map(|line| line.strip_prefix("ARG=").map(ToOwned::to_owned))
        .collect()
}

/// The arguments the strict stub recorded for its version probe.
fn probed_arguments(installation: &Installation) -> Vec<String> {
    let log = std::fs::read_to_string(installation.base().join("stub.probe")).unwrap_or_default();
    log.lines()
        .filter_map(|line| line.strip_prefix("ARG=").map(ToOwned::to_owned))
        .collect()
}

/// The value the stub recorded for `key`, from its most recent invocation.
fn recorded_value(recorded: &[String], key: &str) -> String {
    let prefix = format!("{key}=");
    let Some(value) = recorded
        .iter()
        .rev()
        .find_map(|line| line.strip_prefix(&prefix))
    else {
        panic!("the stub recorded no {key} in {recorded:?}");
    };
    value.to_owned()
}

/// Assert the child saw the bootstrap paths the installation recorded.
fn assert_bootstrap_environment(installation: &Installation) {
    let layout = installation.layout();
    let recorded = installation.recorded();
    assert_eq!(
        recorded_value(&recorded, "LIB"),
        installation.data_root.join("whitaker/lints/lib").as_str(),
        "the lister exports the installed library directory"
    );
    assert_eq!(
        recorded_value(&recorded, "CLIPPY"),
        layout.snapshot(&installation.commit).as_str(),
        "the lister exports the installed snapshot"
    );
    assert_eq!(
        recorded_value(&recorded, "DRIVER"),
        layout.driver_dir("6.3.0", &installation.commit).as_str(),
        "the lister exports the driver slot"
    );
}

#[test]
fn generated_wrapper_lists_only_the_whitaker_suite() {
    let fixture = LsFixture::new().expect("create the lister fixture");

    let output = fixture.run().expect("run the generated lister");

    assert_succeeded(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "whitaker_suite    nightly    /lib\n",
        "the lister keeps the exact suite row and drops the unrelated names"
    );
    assert_eq!(
        recorded_arguments(&fixture.installation),
        ["dylint", "list"],
        "the wrapper passes no unsupported flag to cargo dylint list"
    );
    assert_bootstrap_environment(&fixture.installation);
}

#[test]
fn generated_wrapper_keeps_the_list_status() {
    let fixture = LsFixture::new().expect("create the lister fixture");

    let output = fixture
        .command()
        .env("STUB_EXIT", "7")
        .output()
        .expect("run the generated lister");

    assert_eq!(
        output.status.code(),
        Some(7),
        "pipefail keeps the installer's status: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "whitaker_suite    nightly    /lib\n",
        "the matching row survives a failing list"
    );
}

#[test]
fn generated_wrapper_reports_a_version_mismatch_without_running_list() {
    let fixture = LsFixture::new().expect("create the lister fixture");

    let output = fixture
        .command()
        .env("STUB_DYLINT_VERSION", "6.0.1")
        .output()
        .expect("run the generated lister");

    assert_eq!(
        output.status.code(),
        Some(1),
        "a mismatched cargo-dylint fails the run"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reports version 6.0.1"), "{stderr}");
    assert!(
        recorded_arguments(&fixture.installation).is_empty(),
        "cargo dylint list must not run after the version probe fails"
    );
    assert_eq!(
        probed_arguments(&fixture.installation),
        ["dylint", "--version"],
        "the run fails at the version probe"
    );
}
