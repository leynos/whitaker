//! End-to-end behaviour of the `whitaker-clippy-snapshot` binary.
//!
//! Each test runs the real binary against a small fixture snapshot, pointed
//! at it with the debug-only manifest override, so every subcommand parses
//! its arguments, does its work, and reports through its exit status and
//! streams exactly as the producer's Make targets and CI rely on. Nothing
//! touches the network: `install` takes the fixture bundle, and `build`
//! clones the fixture's local source repository.

use std::process::{Command, Output};

use camino::{Utf8Path, Utf8PathBuf};
use tempfile::TempDir;
use whitaker_installer::{
    clippy_snapshot::METADATA_FILE,
    test_support::{EnvTestGuard, TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV, env_test_guard},
    test_utils::{SnapshotFixture, fixture_git},
};

/// A fixture snapshot and a scratch directory for the binary's output.
///
/// The environment guard is held for the whole life of the value: the fixture
/// runs Git, and every launched binary inherits this process's environment, so
/// a test that set `GIT_DIR` would otherwise affect any concurrent fixture or
/// child process here.
struct Cli {
    fixture: SnapshotFixture,
    _scratch: TempDir,
    scratch: Utf8PathBuf,
    _env_guard: EnvTestGuard,
}

impl Cli {
    /// Build the fixture and a scratch directory, holding the environment lock.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixture or scratch directory cannot be created.
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let env_guard = env_test_guard();
        let fixture = SnapshotFixture::new()?;
        let scratch_dir = TempDir::new()?;
        let scratch = Utf8PathBuf::from_path_buf(scratch_dir.path().to_path_buf())
            .map_err(|path| format!("scratch directory is not UTF-8: {}", path.display()))?;
        Ok(Self {
            fixture,
            _scratch: scratch_dir,
            scratch,
            _env_guard: env_guard,
        })
    }

    /// Run the binary with `args`, pinned to the fixture's manifest.
    ///
    /// # Errors
    ///
    /// Returns an error if the binary cannot be launched or awaited.
    fn run(&self, args: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
        Ok(Command::new(env!("CARGO_BIN_EXE_whitaker-clippy-snapshot"))
            .args(args)
            .env(
                TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV,
                self.fixture.manifest_path(),
            )
            .output()?)
    }

    /// Install the fixture bundle under `root`, returning the printed path.
    ///
    /// # Errors
    ///
    /// Returns an error if the binary cannot be run or reports failure.
    fn install(&self, root: &Utf8Path) -> Result<Utf8PathBuf, Box<dyn std::error::Error>> {
        let output = self.run(&[
            "install",
            "--root",
            root.as_str(),
            "--bundle",
            self.fixture.bundle().as_str(),
        ])?;
        if !output.status.success() {
            return Err(stderr(&output).into());
        }
        Ok(Utf8PathBuf::from(stdout(&output).trim()))
    }
}

/// Return the process's standard output as text.
fn stdout(output: &Output) -> String { String::from_utf8_lossy(&output.stdout).into_owned() }

/// Return the process's standard error as text.
fn stderr(output: &Output) -> String { String::from_utf8_lossy(&output.stderr).into_owned() }

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
fn path_prints_the_generation_without_installing_it() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;
    let root = cli.scratch.join("clippy");

    let output = cli.run(&["path", "--root", root.as_str()])?;

    assert_path_generation(&output, &root, cli.fixture.commit());
    Ok(())
}

/// Check `path` printed the generation without creating it.
fn assert_path_generation(output: &Output, root: &Utf8Path, commit: &str) {
    assert!(output.status.success(), "{}", stderr(output));
    assert_eq!(
        stdout(output).trim(),
        root.join(commit).as_str(),
        "`path` should print the generation it would install"
    );
    assert!(!root.exists(), "`path` is a query and creates nothing");
}

/// Check that `install` persists a verified generation and prints its path.
#[test]
fn install_persists_a_verified_generation_and_prints_its_path()
-> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;
    let root = cli.scratch.join("clippy");

    let generation = cli.install(&root)?;

    let metadata = std::fs::read_to_string(generation.join(METADATA_FILE))?;
    let head = fixture_git(&generation, &["rev-parse", "refs/heads/main"])?;
    assert_generation_path(&generation, &root, cli.fixture.commit());
    assert_generation_metadata(&metadata, &head, cli.fixture.commit());
    Ok(())
}

/// Check `install` persisted the generation at the printed path.
fn assert_generation_path(generation: &Utf8Path, root: &Utf8Path, commit: &str) {
    assert!(generation.is_absolute(), "{generation}");
    assert_eq!(
        generation,
        root.join(commit),
        "the generation should live under the requested root"
    );
}

/// Check the persisted generation records the fixture commit.
fn assert_generation_metadata(metadata: &str, head: &str, commit: &str) {
    assert!(
        metadata.contains(commit),
        "the metadata should record {commit}: {metadata}"
    );
    assert_eq!(
        head.trim(),
        commit,
        "the persisted repository should be at the fixture commit"
    );
}

/// Check that installing twice reuses the first generation.
#[test]
fn a_second_install_reuses_the_generation() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;
    let root = cli.scratch.join("clippy");
    let first = cli.install(&root)?;

    let second = cli.install(&root)?;

    assert_same_generation(&first, &second);
    Ok(())
}

/// Check both installs printed the same generation.
fn assert_same_generation(first: &Utf8Path, second: &Utf8Path) {
    assert_eq!(
        first, second,
        "a second install should reuse the first generation"
    );
}

/// Check that `verify` accepts an installed generation without output.
#[test]
fn verify_accepts_an_installed_generation_silently() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;
    let generation = cli.install(&cli.scratch.join("clippy"))?;

    let output = cli.run(&["verify", "--repo", generation.as_str()])?;

    assert_silent_success(&output);
    Ok(())
}

/// Check a run succeeded and printed nothing.
fn assert_silent_success(output: &Output) {
    assert!(output.status.success(), "{}", stderr(output));
    assert!(
        stdout(output).is_empty() && stderr(output).is_empty(),
        "a successful `verify` should print nothing"
    );
}

/// Check no generation was installed under `root`.
fn assert_nothing_installed(root: &Utf8Path, commit: &str) {
    assert!(
        !root.join(commit).exists(),
        "nothing should be installed under {root}"
    );
}

/// Check `build` printed a bundle inside `output_dir` with a sidecar.
fn assert_built_bundle(output: &Output, output_dir: &Utf8Path) -> Utf8PathBuf {
    assert!(output.status.success(), "{}", stderr(output));
    let bundle = Utf8PathBuf::from(stdout(output).trim());
    assert!(
        bundle.starts_with(output_dir),
        "the bundle should live under {output_dir}: {bundle}"
    );
    assert!(
        Utf8PathBuf::from(format!("{bundle}.sha256")).is_file(),
        "the sidecar should exist for {bundle}"
    );
    bundle
}

/// Check a command succeeded without constraining its output.
fn assert_succeeded(output: &Output) {
    assert!(output.status.success(), "{}", stderr(output));
}

/// Check an unknown subcommand is reported as a usage error.
fn assert_usage_error(output: &Output) {
    assert_eq!(output.status.code(), Some(2), "{}", stderr(output));
    assert!(
        stderr(output).contains("Usage:"),
        "the usage text is missing from: {}",
        stderr(output)
    );
}

/// Check `path` resolved a relative root beneath the working directory.
fn assert_relative_root_path(output: &Output, scratch: &Utf8Path, commit: &str) {
    assert_succeeded(output);
    let printed = Utf8PathBuf::from(stdout(output).trim());
    assert!(printed.is_absolute(), "{printed}");
    assert_eq!(
        printed,
        scratch.join("clippy").join(commit),
        "the relative root should resolve under the working directory"
    );
}

/// Check the fixture neither used nor created the decoy `GIT_DIR`.
fn assert_fixture_ignores_decoy(fixture: &SnapshotFixture, decoy: &Utf8Path) {
    assert!(!decoy.exists(), "the inherited GIT_DIR was not used");
    assert_eq!(
        fixture.commit().len(),
        40,
        "the fixture commit should be a full SHA"
    );
}

/// Check that `verify` rejects a generation that gained a remote.
#[test]
fn verify_rejects_a_generation_that_gained_a_remote() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;
    let generation = cli.install(&cli.scratch.join("clippy"))?;
    fixture_git(
        &generation,
        &["remote", "add", "upstream", "https://example.invalid/x"],
    )?;

    let output = cli.run(&["verify", "--repo", generation.as_str()])?;

    assert_fails_with(&output, "it has remotes");
    Ok(())
}

/// Check that `install` reports a missing bundle and installs nothing.
#[test]
fn install_reports_a_missing_bundle() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;
    let missing = cli.scratch.join("absent.bundle");

    let output = cli.run(&[
        "install",
        "--root",
        cli.scratch.join("clippy").as_str(),
        "--bundle",
        missing.as_str(),
    ])?;

    assert_fails_with(&output, "is missing");
    assert_nothing_installed(&cli.scratch.join("clippy"), cli.fixture.commit());
    Ok(())
}

/// Check that `install` refuses a bundle whose sidecar disagrees.
#[test]
fn install_rejects_a_bundle_whose_sidecar_disagrees() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;
    let bundle = cli.scratch.join("tampered.bundle");
    std::fs::copy(cli.fixture.bundle(), &bundle)?;
    std::fs::write(
        format!("{bundle}.sha256"),
        format!("{}  tampered.bundle\n", "0".repeat(64)),
    )?;
    let root = cli.scratch.join("clippy");

    let output = cli.run(&[
        "install",
        "--root",
        root.as_str(),
        "--bundle",
        bundle.as_str(),
    ])?;

    assert_fails_with(&output, "failed SHA-256 verification");
    assert_nothing_installed(&root, cli.fixture.commit());
    Ok(())
}

/// Check that `build` writes a bundle and sidecar that `install` accepts.
#[test]
fn build_writes_a_bundle_and_sidecar_that_install_accepts() -> Result<(), Box<dyn std::error::Error>>
{
    let cli = Cli::new()?;
    let output_dir = cli.scratch.join("dist");
    let source = cli.fixture.source();

    let output = cli.run(&[
        "build",
        "--output-dir",
        output_dir.as_str(),
        "--source",
        source.as_str(),
    ])?;

    let bundle = assert_built_bundle(&output, &output_dir);
    let root = cli.scratch.join("clippy");
    let installed = cli.run(&[
        "install",
        "--root",
        root.as_str(),
        "--bundle",
        bundle.as_str(),
    ])?;
    assert_succeeded(&installed);
    Ok(())
}

/// Check that an unknown subcommand is a usage error.
#[test]
fn an_unknown_subcommand_is_a_usage_error() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;

    let output = cli.run(&["publish"])?;

    assert_usage_error(&output);
    Ok(())
}

/// Check that `path` resolves a relative root, as `install` does.
#[test]
fn path_resolves_a_relative_root() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::new()?;

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker-clippy-snapshot"))
        .args(["path", "--root", "clippy"])
        .current_dir(&cli.scratch)
        .env(
            TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV,
            cli.fixture.manifest_path(),
        )
        .output()?;

    assert_relative_root_path(&output, &cli.scratch, cli.fixture.commit());
    Ok(())
}

/// Check that the fixture's Git ignores an inherited `GIT_DIR`.
#[test]
fn the_fixture_ignores_an_inherited_git_dir() -> Result<(), Box<dyn std::error::Error>> {
    let elsewhere = TempDir::new()?;
    let decoy = Utf8PathBuf::from_path_buf(elsewhere.path().join("decoy.git"))
        .map_err(|path| format!("decoy path is not UTF-8: {}", path.display()))?;
    let _guard = whitaker_installer::test_support::env_test_guard();

    let built = temp_env::with_var("GIT_DIR", Some(decoy.as_str()), SnapshotFixture::new);

    let fixture = built?;
    assert_fixture_ignores_decoy(&fixture, &decoy);
    Ok(())
}
