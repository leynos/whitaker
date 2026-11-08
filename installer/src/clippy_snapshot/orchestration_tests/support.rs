//! Scripted ports and manifest fixtures shared by the orchestration tests.

use std::sync::{Arc, Mutex};

use camino::Utf8PathBuf;

use super::super::{
    fakes::{FakeBundles, FakeMetadata, FakeRepository, FakeStore, UnusedDownloader},
    generation::GenerationMetadata,
    install::BundleArtefact,
    manifest::{ClippySnapshotManifest, parse_manifest},
    policy::{RefEntry, SelfContainedEvidence, StructureEvidence},
    ports::SnapshotSupport,
};
use crate::artefact::sha256_digest::Sha256Digest;

pub(super) const TIP: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(super) const SELECTED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub(super) const OLDER: &str = "cccccccccccccccccccccccccccccccccccccccc";
pub(super) const OTHER: &str = "dddddddddddddddddddddddddddddddddddddddd";
const PINNED: &str = "nightly-2026-05-28";
const NEWER: &str = "nightly-2026-06-11";
const OLDEST: &str = "nightly-2026-05-13";

/// A digest that is valid but does not matter to the fake store.
pub(super) fn digest(hex: char) -> Sha256Digest {
    Sha256Digest::try_from(hex.to_string().repeat(64).as_str()).expect("64 hexadecimal digits")
}

/// The manifest every orchestration test pins.
pub(super) fn manifest() -> ClippySnapshotManifest {
    parse_manifest(&format!(
        concat!(
            "schema_version = 1\n",
            "upstream = \"https://example.invalid/rust-clippy\"\n",
            "branch = \"main\"\n",
            "commit = \"{TIP}\"\n",
            "toolchain = \"{PINNED}\"\n",
            "selected_revision = \"{SELECTED}\"\n",
            "dylint_version = \"6.3.0\"\n",
        ),
        TIP = TIP,
        PINNED = PINNED,
        SELECTED = SELECTED,
    ))
    .expect("the orchestration manifest is valid")
}

/// A toolchain file naming `channel`.
fn toolchain(channel: &str) -> String { format!("[toolchain]\nchannel = \"{channel}\"\n") }

/// A repository whose every observation supports the pinned manifest.
fn valid_repository() -> FakeRepository {
    let mut repo = FakeRepository::new();
    repo.structure = Ok(StructureEvidence {
        is_bare: "true".to_owned(),
        head: "refs/heads/main".to_owned(),
        refs: vec![RefEntry {
            name: "refs/heads/main".to_owned(),
            object: TIP.to_owned(),
        }],
    });
    repo.self_contained = Ok(SelfContainedEvidence {
        is_shallow: "false".to_owned(),
        remotes: Vec::new(),
    });
    repo.commits = Ok(vec![TIP.to_owned(), SELECTED.to_owned(), OLDER.to_owned()]);
    repo.add_file(TIP, "rust-toolchain.toml", &toolchain(NEWER))
        .add_file(SELECTED, "rust-toolchain.toml", &toolchain(PINNED))
        .add_file(OLDER, "rust-toolchain.toml", &toolchain(OLDEST));
    repo
}

/// A bundle artefact whose digest the fake store reproduces.
pub(super) fn artefact() -> BundleArtefact {
    BundleArtefact {
        bundle: Utf8PathBuf::from("snapshot.bundle"),
        sha256: digest('a'),
    }
}

/// The head listing a well-formed bundle carries.
fn pinned_heads() -> String { format!("{TIP} refs/heads/main\n{TIP} HEAD") }

/// Everything one operation needs, owned by the caller for the duration.
pub(super) struct Ports {
    pub(super) repo: FakeRepository,
    pub(super) bundles: FakeBundles,
    pub(super) store: FakeStore,
    pub(super) metadata: FakeMetadata,
    /// Store and metadata calls in the order they happened.
    shared: Arc<Mutex<Vec<String>>>,
}

impl Ports {
    /// Ports that would accept a consistent snapshot and bundle.
    pub(super) fn accepting() -> Self {
        let mut bundles = FakeBundles::new();
        bundles.heads = Ok(pinned_heads());
        let shared = Arc::new(Mutex::new(Vec::new()));
        Self {
            repo: valid_repository(),
            bundles,
            store: FakeStore::new(digest('a')).sharing(&shared),
            metadata: FakeMetadata::new(GenerationMetadata::for_manifest(
                &manifest(),
                digest('a').as_str(),
            ))
            .sharing(&shared),
            shared,
        }
    }

    /// Return the store and metadata calls in the order they happened.
    pub(super) fn calls(&self) -> Vec<String> {
        self.shared.lock().expect("lock shared log").clone()
    }

    /// Borrow the ports as one support bundle.
    pub(super) fn support(&self) -> SnapshotSupport<'_> {
        SnapshotSupport {
            repository: &self.repo,
            bundles: &self.bundles,
            store: &self.store,
            metadata: &self.metadata,
            downloader: &UnusedDownloader,
        }
    }
}
