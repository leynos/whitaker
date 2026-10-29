//! Installation and verification of snapshot bundles against local fixtures.

use super::test_fixtures::{
    DEFAULT_HISTORY, DEFAULT_SELECTED, FixtureRepo, Step, git, write_sidecar,
};
use super::*;
use rstest::{fixture, rstest};

const PINNED_REFS: [&str; 2] = ["refs/heads/main", "HEAD"];

/// A default fixture with its bundle, the manifest pinning its tip, and a
/// fresh snapshot root.
struct Published {
    repo: FixtureRepo,
    bundle: Utf8PathBuf,
    manifest: ClippySnapshotManifest,
    root: Utf8PathBuf,
}

impl Published {
    /// Install the published bundle, as a consumer would.
    fn install(&self) -> Result<Utf8PathBuf, ClippySnapshotError> {
        let artefact = BundleArtefact::from_sidecar(&self.bundle, &self.manifest)?;
        install_bundle(&artefact, &self.manifest, &self.root)
    }

    /// Where the pinned generation would be installed.
    fn generation(&self) -> Utf8PathBuf {
        generation_path(&self.root, &self.manifest)
    }
}

#[fixture]
fn published() -> Published {
    let repo = FixtureRepo::new(&DEFAULT_HISTORY);
    let bundle = repo.bundle("snapshot.bundle", &PINNED_REFS);
    let manifest = repo.manifest(repo.tip(), &repo.commits[DEFAULT_SELECTED]);
    let root = repo.path("root");
    Published {
        repo,
        bundle,
        manifest,
        root,
    }
}

#[rstest]
fn installs_a_main_only_snapshot(published: Published) {
    let generation = published.install().expect("a valid bundle installs");

    assert_eq!(generation, published.generation());
    assert!(
        generation.is_absolute(),
        "DYLINT_CLIPPY_REPO must be absolute"
    );
    let refs = git(&generation, &["for-each-ref", "--format=%(refname)"]);
    assert_eq!(refs, "refs/heads/main", "no master, no other refs");
    assert_eq!(
        git(&generation, &["remote"]),
        "",
        "the clone keeps no remote"
    );
    check_installed(&generation, published.manifest.commit.as_str())
        .expect("the warm-path check accepts a fresh generation");
    let metadata = GenerationMetadata::read(&generation).expect("metadata is written");
    metadata
        .check_matches(&generation, &published.manifest)
        .expect("metadata describes the pinned snapshot");
}

#[rstest]
fn reuses_an_existing_generation_without_rewriting_it(published: Published) {
    let generation = published.install().expect("first install");
    let metadata = generation.join(METADATA_FILE);
    let before = std::fs::metadata(&metadata)
        .and_then(|m| m.modified())
        .expect("mtime");

    let again = published
        .install()
        .expect("second install reuses the generation");

    assert_eq!(again, generation);
    let after = std::fs::metadata(&metadata)
        .and_then(|m| m.modified())
        .expect("mtime");
    assert_eq!(before, after, "an accepted generation is never rewritten");
}

/// Every rejected bundle leaves no directory under the generation's name.
#[rstest]
#[case::missing_bundle(Corruption::DeleteBundle, "is missing")]
#[case::missing_sidecar(Corruption::DeleteSidecar, "sidecar")]
#[case::tampered_bytes(Corruption::FlipByte, "failed SHA-256 verification")]
#[case::truncated_bundle(Corruption::Truncate, "the bundle could not be cloned")]
fn rejects_corrupt_bundles(
    published: Published,
    #[case] corruption: Corruption,
    #[case] expected: &str,
) {
    corruption.apply(&published.bundle);

    let error = published
        .install()
        .expect_err("a corrupt bundle is rejected");

    assert_rejected(&published, &error, expected);
}

#[rstest]
fn rejects_a_bundle_that_also_carries_master(published: Published) {
    git(&published.repo.work, &["branch", "master"]);
    let bundle = published.repo.bundle(
        "with-master.bundle",
        &["refs/heads/main", "refs/heads/master", "HEAD"],
    );
    let artefact = BundleArtefact::from_sidecar(&bundle, &published.manifest).expect("sidecar");

    let error = install_bundle(&artefact, &published.manifest, &published.root)
        .expect_err("extra refs are rejected");

    assert_rejected(&published, &error, "refs/heads/master");
}

#[rstest]
fn rejects_a_bundle_for_a_different_commit(published: Published) {
    let manifest = published.repo.manifest(
        &published.repo.commits[5],
        &published.repo.commits[DEFAULT_SELECTED],
    );
    let artefact = BundleArtefact::from_sidecar(&published.bundle, &manifest).expect("sidecar");

    let error = install_bundle(&artefact, &manifest, &published.root)
        .expect_err("a bundle for another commit is rejected");

    assert!(error.to_string().contains("must carry only"), "{error}");
    assert!(!generation_path(&published.root, &manifest).exists());
}

/// A bundle with prerequisites lacks the history Dylint walks.
#[rstest]
fn rejects_a_bundle_cut_above_the_selected_revision(published: Published) {
    let cut = format!("^{}", published.repo.commits[DEFAULT_SELECTED]);
    let bundle = published
        .repo
        .bundle("cut.bundle", &["refs/heads/main", "HEAD", &cut]);
    let artefact = BundleArtefact::from_sidecar(&bundle, &published.manifest).expect("sidecar");

    let error = install_bundle(&artefact, &published.manifest, &published.root)
        .expect_err("a bundle with prerequisites is rejected");

    assert_rejected(&published, &error, "git bundle verify failed");
}

/// Complete history whose oldest channel is the pinned one cannot show
/// where that channel began, so Dylint would select nothing.
#[test]
fn rejects_history_without_the_channel_introduction() {
    let repo = FixtureRepo::new(&[Step::Channel("nightly-2026-05-28"), Step::Unrelated]);
    let bundle = repo.bundle("short.bundle", &PINNED_REFS);
    let manifest = repo.manifest(repo.tip(), &repo.commits[0]);
    let root = repo.path("root");
    let artefact = BundleArtefact::from_sidecar(&bundle, &manifest).expect("sidecar");

    let error = install_bundle(&artefact, &manifest, &root).expect_err("insufficient history");

    assert!(
        matches!(error, ClippySnapshotError::InsufficientHistory { .. }),
        "{error:?}"
    );
    assert!(!generation_path(&root, &manifest).exists());
}

#[rstest]
fn rejects_a_manifest_recording_a_different_selection(published: Published) {
    let manifest = published
        .repo
        .manifest(published.repo.tip(), &published.repo.commits[1]);
    let artefact = BundleArtefact::from_sidecar(&published.bundle, &manifest).expect("sidecar");

    let error = install_bundle(&artefact, &manifest, &published.root)
        .expect_err("a wrong recorded selection is rejected");

    assert!(error.to_string().contains("Dylint would select"), "{error}");
}

#[rstest]
fn refuses_to_reuse_a_damaged_generation(published: Published) {
    let generation = published.install().expect("first install");
    git(
        &generation,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/rust-lang/rust-clippy",
        ],
    );

    let error = published
        .install()
        .expect_err("a damaged generation is not reused");

    assert!(error.to_string().contains("it has remotes"), "{error}");
    assert_eq!(
        git(&generation, &["remote"]),
        "origin",
        "installation never repairs a generation in place"
    );
}

#[rstest]
fn the_warm_check_rejects_a_missing_generation(published: Published) {
    let error = check_installed(&published.generation(), published.manifest.commit.as_str())
        .expect_err("nothing is installed");

    assert!(
        matches!(error, ClippySnapshotError::NotInstalled { .. }),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("never falls back to cloning public Clippy")
    );
}

/// Ways to damage a published bundle or its sidecar.
#[derive(Debug, Clone, Copy)]
enum Corruption {
    DeleteBundle,
    DeleteSidecar,
    FlipByte,
    Truncate,
}

impl Corruption {
    /// Damage the files at `bundle`.
    fn apply(self, bundle: &Utf8Path) {
        match self {
            Self::DeleteBundle => std::fs::remove_file(bundle).expect("delete bundle"),
            Self::DeleteSidecar => {
                std::fs::remove_file(format!("{bundle}.sha256")).expect("delete sidecar");
            }
            Self::FlipByte => {
                let mut bytes = std::fs::read(bundle).expect("read bundle");
                let last = bytes.len() - 1;
                bytes[last] ^= 0xff;
                std::fs::write(bundle, bytes).expect("write bundle");
            }
            Self::Truncate => {
                let bytes = std::fs::read(bundle).expect("read bundle");
                std::fs::write(bundle, &bytes[..bytes.len() / 2]).expect("write bundle");
                // Re-publish the digest so only Git's own checks can object.
                write_sidecar(bundle);
            }
        }
    }
}

/// Require a rejection mentioning `expected` that left no generation behind.
fn assert_rejected(published: &Published, error: &ClippySnapshotError, expected: &str) {
    assert!(
        error.to_string().contains(expected),
        "`{error}` should mention `{expected}`"
    );
    assert!(
        !published.generation().exists(),
        "a failed install must not leave a generation"
    );
}
