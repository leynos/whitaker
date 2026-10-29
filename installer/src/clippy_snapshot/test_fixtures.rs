//! Local Git histories that stand in for Clippy in snapshot tests.
//!
//! Each fixture is a small `main`-only repository whose commits move the
//! `rust-toolchain.toml` channel the way Clippy's rustup commits do. Tests
//! build bundles from it, corrupt them, and check what installation and
//! verification make of the result, all without network access.

use super::manifest::{ClippySnapshotManifest, parse_manifest};
use camino::{Utf8Path, Utf8PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// The nightly the fixtures pin, as Whitaker pins its own.
pub(super) const FIXTURE_TOOLCHAIN: &str = "nightly-2026-05-28";

/// One step of fixture history: a toolchain move, or an unrelated edit.
#[derive(Debug, Clone, Copy)]
pub(super) enum Step {
    /// Commit a toolchain file naming this channel.
    Channel(&'static str),
    /// Commit an edit that leaves the toolchain file alone.
    Unrelated,
}

/// The default history, oldest first. `nightly-2026-05-28` is introduced by
/// the fourth commit ([`DEFAULT_SELECTED`]), whose first parent is on
/// `nightly-2026-05-13`.
pub(super) const DEFAULT_HISTORY: [Step; 7] = [
    Step::Channel("nightly-2026-04-30"),
    Step::Channel("nightly-2026-05-13"),
    Step::Unrelated,
    Step::Channel(FIXTURE_TOOLCHAIN),
    Step::Unrelated,
    Step::Channel("nightly-2026-06-11"),
    Step::Unrelated,
];

/// The index in [`DEFAULT_HISTORY`] of the commit Dylint selects.
pub(super) const DEFAULT_SELECTED: usize = 3;

/// A fixture work tree and the commits it recorded, oldest first.
pub(super) struct FixtureRepo {
    /// Owns every path below; dropped with the fixture.
    pub(super) dir: TempDir,
    /// The non-bare work tree the history was committed in.
    pub(super) work: Utf8PathBuf,
    /// Commit object IDs, oldest first.
    pub(super) commits: Vec<String>,
}

impl FixtureRepo {
    /// Commit `steps` in a fresh repository on `main`.
    pub(super) fn new(steps: &[Step]) -> Self {
        let dir = TempDir::new().expect("create fixture directory");
        let work = utf8(dir.path()).join("work");
        std::fs::create_dir_all(work.join("clippy_utils")).expect("create work tree");
        git(&work, &["init", "--quiet", "--initial-branch=main"]);
        let mut fixture = Self {
            dir,
            work,
            commits: Vec::new(),
        };
        for (index, step) in steps.iter().enumerate() {
            fixture.commit(index, *step);
        }
        fixture
    }

    /// The newest commit.
    pub(super) fn tip(&self) -> &str {
        self.commits.last().expect("fixture has commits")
    }

    /// A scratch path inside the fixture directory.
    pub(super) fn path(&self, name: &str) -> Utf8PathBuf {
        utf8(self.dir.path()).join(name)
    }

    /// Write a bundle of `refs` (for example `["refs/heads/main", "HEAD"]`)
    /// and its sidecar, returning the bundle path.
    pub(super) fn bundle(&self, name: &str, refs: &[&str]) -> Utf8PathBuf {
        let bundle = self.path(name);
        let mut args = vec!["bundle", "create", "--quiet", bundle.as_str()];
        args.extend(refs);
        git(&self.work, &args);
        write_sidecar(&bundle);
        bundle
    }

    /// A manifest pinning `commit`, selecting `selected`, for the fixture nightly.
    pub(super) fn manifest(&self, commit: &str, selected: &str) -> ClippySnapshotManifest {
        parse_manifest(&format!(
            concat!(
                "schema_version = 1\n",
                "upstream = \"https://example.invalid/rust-clippy\"\n",
                "branch = \"main\"\n",
                "commit = \"{commit}\"\n",
                "toolchain = \"{toolchain}\"\n",
                "selected_revision = \"{selected}\"\n",
                "dylint_version = \"6.3.0\"\n",
            ),
            commit = commit,
            toolchain = FIXTURE_TOOLCHAIN,
            selected = selected,
        ))
        .expect("fixture manifest is valid")
    }

    /// Record one step as a commit.
    fn commit(&mut self, index: usize, step: Step) {
        let cargo = format!("[package]\nname = \"clippy_utils\"\nversion = \"0.1.{index}\"\n");
        std::fs::write(self.work.join("clippy_utils/Cargo.toml"), cargo).expect("write Cargo.toml");
        match step {
            Step::Channel(channel) => {
                let toolchain = format!("[toolchain]\nchannel = \"{channel}\"\n");
                std::fs::write(self.work.join("rust-toolchain.toml"), toolchain)
                    .expect("write toolchain");
            }
            Step::Unrelated => {
                std::fs::write(self.work.join("README.md"), format!("step {index}\n"))
                    .expect("write README");
            }
        }
        git(&self.work, &["add", "--all"]);
        let message = format!("step {index}");
        git(&self.work, &["commit", "--quiet", "--message", &message]);
        self.commits.push(git(&self.work, &["rev-parse", "HEAD"]));
    }
}

/// Write `<bundle>.sha256` in `sha256sum` format.
pub(super) fn write_sidecar(bundle: &Utf8Path) {
    let bytes = std::fs::read(bundle).expect("read bundle");
    let digest = crate::test_utils::sha256_hex(&bytes);
    let name = bundle.file_name().expect("bundle has a name");
    std::fs::write(format!("{bundle}.sha256"), format!("{digest}  {name}\n"))
        .expect("write sidecar");
}

/// An empty configuration file, so the developer's Git settings never leak in.
#[cfg(windows)]
const NULL_DEVICE: &str = "NUL";
#[cfg(not(windows))]
const NULL_DEVICE: &str = "/dev/null";

/// Run fixture Git with a fixed identity and no ambient configuration.
pub(super) fn git(dir: &Utf8Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", NULL_DEVICE)
        .env("GIT_CONFIG_COUNT", "0")
        .env("GIT_AUTHOR_DATE", "2026-05-28T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-05-28T00:00:00Z")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// View a temporary path as UTF-8.
pub(super) fn utf8(path: &std::path::Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path.to_path_buf()).expect("temporary paths are UTF-8")
}

/// Express absolute `path` relative to the current directory.
///
/// The path climbs only to the deepest directory shared with the current
/// directory, so it resolves correctly from here and wrongly from anywhere
/// else, which is what the relative-input regressions need. A path that
/// climbed to `/` would resolve identically from every directory.
#[cfg(unix)]
pub(super) fn relative_to_cwd(path: &Utf8Path) -> Utf8PathBuf {
    let cwd = utf8(&std::env::current_dir().expect("current directory"));
    let shared = cwd
        .components()
        .zip(path.components())
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = Utf8PathBuf::new();
    for _ in shared..cwd.components().count() {
        relative.push("..");
    }
    for component in path.components().skip(shared) {
        relative.push(component.as_str());
    }
    relative
}

/// A scratch directory inside the build tree, which shares an ancestor with
/// the current directory, unlike the system temporary directory.
#[cfg(unix)]
pub(super) fn build_tree_scratch() -> TempDir {
    let exe = std::env::current_exe().expect("test executable path");
    let parent = exe.parent().expect("test executable has a parent");
    tempfile::Builder::new()
        .prefix("clippy-snapshot-relative-")
        .tempdir_in(parent)
        .expect("create scratch directory in the build tree")
}
