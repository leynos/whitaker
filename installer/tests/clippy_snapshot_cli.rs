//! End-to-end behaviour of the `whitaker-clippy-snapshot` binary.
//!
//! Each test runs the real binary against a small fixture snapshot, pointed
//! at it with the debug-only manifest override, so every subcommand parses
//! its arguments, does its work, and reports through its exit status and
//! streams exactly as the producer's Make targets and CI rely on. Nothing
//! touches the network: `install` takes the fixture bundle, and `build`
//! clones the fixture's local source repository.

use camino::{Utf8Path, Utf8PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;
use whitaker_installer::clippy_snapshot::METADATA_FILE;
use whitaker_installer::test_support::TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV;
use whitaker_installer::test_utils::{SnapshotFixture, fixture_git};

/// A fixture snapshot and a scratch directory for the binary's output.
struct Cli {
    fixture: SnapshotFixture,
    _scratch: TempDir,
    scratch: Utf8PathBuf,
}

impl Cli {
    /// Build the fixture and a scratch directory.
    fn new() -> Self {
        let fixture = SnapshotFixture::new().expect("build the snapshot fixture");
        let scratch_dir = TempDir::new().expect("create scratch dir");
        let scratch =
            Utf8PathBuf::from_path_buf(scratch_dir.path().to_path_buf()).expect("UTF-8 scratch");
        Self {
            fixture,
            _scratch: scratch_dir,
            scratch,
        }
    }

    /// Run the binary with `args`, pinned to the fixture's manifest.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_whitaker-clippy-snapshot"))
            .args(args)
            .env(
                TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV,
                self.fixture.manifest_path(),
            )
            .output()
            .expect("run whitaker-clippy-snapshot")
    }

    /// Install the fixture bundle under `root`, returning the printed path.
    fn install(&self, root: &Utf8Path) -> Utf8PathBuf {
        let output = self.run(&[
            "install",
            "--root",
            root.as_str(),
            "--bundle",
            self.fixture.bundle().as_str(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        Utf8PathBuf::from(stdout(&output).trim())
    }
}

/// Return the process's standard output as text.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Return the process's standard error as text.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Require a failed run that names the binary and `expected` on stderr.
fn assert_fails_with(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{}", stderr(output));
    let stderr = stderr(output);
    assert!(stderr.starts_with("whitaker-clippy-snapshot: "), "{stderr}");
    assert!(
        stderr.contains(expected),
        "`{expected}` missing from: {stderr}"
    );
    assert!(
        stdout(output).is_empty(),
        "a failure prints nothing on stdout"
    );
}

/// Check that `path` prints the generation and creates nothing.
#[test]
fn path_prints_the_generation_without_installing_it() {
    let cli = Cli::new();
    let root = cli.scratch.join("clippy");

    let output = cli.run(&["path", "--root", root.as_str()]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        root.join(cli.fixture.commit()).as_str()
    );
    assert!(!root.exists(), "`path` is a query and creates nothing");
}

/// Check that `install` persists a verified generation and prints its path.
#[test]
fn install_persists_a_verified_generation_and_prints_its_path() {
    let cli = Cli::new();
    let root = cli.scratch.join("clippy");

    let generation = cli.install(&root);

    assert!(generation.is_absolute(), "{generation}");
    assert_eq!(generation, root.join(cli.fixture.commit()));
    let metadata =
        std::fs::read_to_string(generation.join(METADATA_FILE)).expect("read generation metadata");
    assert!(metadata.contains(cli.fixture.commit()), "{metadata}");
    let head = fixture_git(&generation, &["rev-parse", "refs/heads/main"]).expect("read main");
    assert_eq!(head.trim(), cli.fixture.commit());
}

/// Check that installing twice reuses the first generation.
#[test]
fn a_second_install_reuses_the_generation() {
    let cli = Cli::new();
    let root = cli.scratch.join("clippy");
    let first = cli.install(&root);

    let second = cli.install(&root);

    assert_eq!(first, second);
}

/// Check that `verify` accepts an installed generation without output.
#[test]
fn verify_accepts_an_installed_generation_silently() {
    let cli = Cli::new();
    let generation = cli.install(&cli.scratch.join("clippy"));

    let output = cli.run(&["verify", "--repo", generation.as_str()]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).is_empty() && stderr(&output).is_empty());
}

/// Check that `verify` rejects a generation that gained a remote.
#[test]
fn verify_rejects_a_generation_that_gained_a_remote() {
    let cli = Cli::new();
    let generation = cli.install(&cli.scratch.join("clippy"));
    fixture_git(
        &generation,
        &["remote", "add", "upstream", "https://example.invalid/x"],
    )
    .expect("add a remote");

    let output = cli.run(&["verify", "--repo", generation.as_str()]);

    assert_fails_with(&output, "it has remotes");
}

/// Check that `install` reports a missing bundle and installs nothing.
#[test]
fn install_reports_a_missing_bundle() {
    let cli = Cli::new();
    let missing = cli.scratch.join("absent.bundle");

    let output = cli.run(&[
        "install",
        "--root",
        cli.scratch.join("clippy").as_str(),
        "--bundle",
        missing.as_str(),
    ]);

    assert_fails_with(&output, "is missing");
    assert!(
        !cli.scratch
            .join("clippy")
            .join(cli.fixture.commit())
            .exists(),
        "nothing is installed"
    );
}

/// Check that `install` refuses a bundle whose sidecar disagrees.
#[test]
fn install_rejects_a_bundle_whose_sidecar_disagrees() {
    let cli = Cli::new();
    let bundle = cli.scratch.join("tampered.bundle");
    std::fs::copy(cli.fixture.bundle(), &bundle).expect("copy the bundle");
    std::fs::write(
        format!("{bundle}.sha256"),
        format!("{}  tampered.bundle\n", "0".repeat(64)),
    )
    .expect("write a wrong sidecar");
    let root = cli.scratch.join("clippy");

    let output = cli.run(&[
        "install",
        "--root",
        root.as_str(),
        "--bundle",
        bundle.as_str(),
    ]);

    assert_fails_with(&output, "failed SHA-256 verification");
    assert!(
        !root.join(cli.fixture.commit()).exists(),
        "nothing is installed"
    );
}

/// Check that `build` writes a bundle and sidecar that `install` accepts.
#[test]
fn build_writes_a_bundle_and_sidecar_that_install_accepts() {
    let cli = Cli::new();
    let output_dir = cli.scratch.join("dist");
    let source = cli.fixture.source();

    let output = cli.run(&[
        "build",
        "--output-dir",
        output_dir.as_str(),
        "--source",
        source.as_str(),
    ]);

    assert!(output.status.success(), "{}", stderr(&output));
    let bundle = Utf8PathBuf::from(stdout(&output).trim());
    assert!(bundle.starts_with(&output_dir), "{bundle}");
    assert!(Utf8PathBuf::from(format!("{bundle}.sha256")).is_file());
    let root = cli.scratch.join("clippy");
    let installed = cli.run(&[
        "install",
        "--root",
        root.as_str(),
        "--bundle",
        bundle.as_str(),
    ]);
    assert!(installed.status.success(), "{}", stderr(&installed));
}

/// Check that an unknown subcommand is a usage error.
#[test]
fn an_unknown_subcommand_is_a_usage_error() {
    let cli = Cli::new();

    let output = cli.run(&["publish"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).contains("Usage:"), "{}", stderr(&output));
}

/// Check that `path` resolves a relative root, as `install` does.
#[test]
fn path_resolves_a_relative_root() {
    let cli = Cli::new();

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker-clippy-snapshot"))
        .args(["path", "--root", "clippy"])
        .current_dir(&cli.scratch)
        .env(
            TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV,
            cli.fixture.manifest_path(),
        )
        .output()
        .expect("run whitaker-clippy-snapshot");

    assert!(output.status.success(), "{}", stderr(&output));
    let printed = Utf8PathBuf::from(stdout(&output).trim());
    assert!(printed.is_absolute(), "{printed}");
    assert_eq!(
        printed,
        cli.scratch.join("clippy").join(cli.fixture.commit())
    );
}

/// Check that the fixture's Git ignores an inherited `GIT_DIR`.
#[test]
fn the_fixture_ignores_an_inherited_git_dir() {
    let elsewhere = TempDir::new().expect("create a decoy directory");
    let decoy = Utf8PathBuf::from_path_buf(elsewhere.path().join("decoy.git")).expect("UTF-8");
    let _guard = whitaker_installer::test_support::env_test_guard();

    let fixture = temp_env::with_var("GIT_DIR", Some(decoy.as_str()), SnapshotFixture::new);

    let fixture = fixture.expect("the fixture builds in its own work tree");
    assert!(!decoy.exists(), "the inherited GIT_DIR was not used");
    assert_eq!(fixture.commit().len(), 40);
}
