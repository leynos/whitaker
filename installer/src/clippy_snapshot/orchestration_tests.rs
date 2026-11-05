//! Verification and installation orchestration over scripted ports.
//!
//! These tests never run Git or touch the filesystem. They show that the
//! orchestration asks the ports in the intended order, stops at the first
//! failed check, and neither publishes nor records metadata after a failure.

use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;

use super::{
    error::ClippySnapshotError,
    fakes::{FakeBundles, FakeMetadata, FakeRepository, FakeStore, UnusedDownloader},
    generation::GenerationMetadata,
    install::{BundleArtefact, install_with},
    manifest::{ClippySnapshotManifest, parse_manifest},
    policy::{SelfContainedEvidence, StructureEvidence},
    ports::SnapshotSupport,
    verify::{check_installed_with, verify_with},
};
use crate::artefact::sha256_digest::Sha256Digest;

const TIP: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SELECTED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OLDER: &str = "cccccccccccccccccccccccccccccccccccccccc";
const OTHER: &str = "dddddddddddddddddddddddddddddddddddddddd";
const PINNED: &str = "nightly-2026-05-28";
const NEWER: &str = "nightly-2026-06-11";
const OLDEST: &str = "nightly-2026-05-13";

/// A digest that is valid but does not matter to the fake store.
fn digest(hex: char) -> Sha256Digest {
    Sha256Digest::try_from(hex.to_string().repeat(64).as_str()).expect("64 hexadecimal digits")
}

/// The manifest every orchestration test pins.
fn manifest() -> ClippySnapshotManifest {
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
        refs: format!("refs/heads/main {TIP}"),
    });
    repo.self_contained = Ok(SelfContainedEvidence {
        is_shallow: "false".to_owned(),
        remotes: String::new(),
    });
    repo.commits = Ok(vec![TIP.to_owned(), SELECTED.to_owned(), OLDER.to_owned()]);
    repo.add_file(TIP, "rust-toolchain.toml", &toolchain(NEWER))
        .add_file(SELECTED, "rust-toolchain.toml", &toolchain(PINNED))
        .add_file(OLDER, "rust-toolchain.toml", &toolchain(OLDEST));
    repo
}

/// A bundle artefact whose digest the fake store reproduces.
fn artefact() -> BundleArtefact {
    BundleArtefact {
        bundle: Utf8PathBuf::from("snapshot.bundle"),
        sha256: digest('a'),
    }
}

/// The head listing a well-formed bundle carries.
fn pinned_heads() -> String { format!("{TIP} refs/heads/main\n{TIP} HEAD") }

/// Everything one operation needs, owned by the caller for the duration.
struct Ports {
    repo: FakeRepository,
    bundles: FakeBundles,
    store: FakeStore,
    metadata: FakeMetadata,
}

impl Ports {
    /// Ports that would accept a consistent snapshot and bundle.
    fn accepting() -> Self {
        let mut bundles = FakeBundles::new();
        bundles.heads = Ok(pinned_heads());
        Self {
            repo: valid_repository(),
            bundles,
            store: FakeStore::new(digest('a')),
            metadata: FakeMetadata::new(GenerationMetadata::for_manifest(
                &manifest(),
                digest('a').as_str(),
            )),
        }
    }

    /// Borrow the ports as one support bundle.
    fn support(&self) -> SnapshotSupport<'_> {
        SnapshotSupport {
            repository: &self.repo,
            bundles: &self.bundles,
            store: &self.store,
            metadata: &self.metadata,
            downloader: &UnusedDownloader,
        }
    }
}

#[test]
fn a_missing_directory_is_reported_without_querying_git() {
    let path = Utf8Path::new("/snapshot.git");
    let mut ports = Ports::accepting();
    ports.store.present_dir = false;

    let error = verify_with(path, &manifest(), &ports.support())
        .expect_err("an absent generation cannot verify");

    assert_eq!(
        error,
        ClippySnapshotError::NotInstalled {
            path: path.to_string()
        }
    );
    assert!(ports.repo.calls().is_empty(), "{:?}", ports.repo.calls());
    assert_eq!(ports.store.calls(), vec![format!("is_present_dir {path}")]);
}

#[test]
fn verification_stops_after_the_first_rejected_check() {
    let path = Utf8Path::new("/snapshot.git");
    let mut ports = Ports::accepting();
    ports.repo.structure = Ok(StructureEvidence {
        is_bare: "false".to_owned(),
        head: String::new(),
        refs: String::new(),
    });

    let error =
        verify_with(path, &manifest(), &ports.support()).expect_err("a work tree cannot verify");

    assert!(
        matches!(&error, ClippySnapshotError::Invalid { check, .. }
            if check == "it is not a bare repository"),
        "{error}"
    );
    assert_eq!(
        ports.repo.calls(),
        vec![format!("structure {path}")],
        "no later observation may run"
    );
}

#[test]
fn verification_propagates_an_observation_failure() {
    let path = Utf8Path::new("/snapshot.git");
    let mut ports = Ports::accepting();
    ports.repo.self_contained = Err(ClippySnapshotError::Git {
        operation: "remote",
        message: "cannot read remotes".to_owned(),
    });

    let error = verify_with(path, &manifest(), &ports.support())
        .expect_err("an unreadable repository cannot verify");

    assert!(
        matches!(&error, ClippySnapshotError::Git { operation, .. }
            if *operation == "remote"),
        "{error}"
    );
    assert_eq!(
        ports.repo.calls(),
        vec![
            format!("structure {path}"),
            format!("self_contained {path}"),
        ]
    );
    assert!(
        !ports.store.has_call_starting_with("is_present "),
        "the alternates probe follows the Git query, not the other way round"
    );
}

#[test]
fn verification_observes_each_check_in_the_documented_order() {
    let path = Utf8Path::new("/snapshot.git");
    let ports = Ports::accepting();

    verify_with(path, &manifest(), &ports.support()).expect("a consistent snapshot verifies");

    let calls = ports.repo.calls();
    let position = |prefix: &str| {
        calls
            .iter()
            .position(|call| call.starts_with(prefix))
            .unwrap_or_else(|| panic!("missing {prefix}: {calls:?}"))
    };
    let documented = [
        "structure ",
        "self_contained ",
        "integrity ",
        "toolchain_commits ",
    ];
    let observed: Vec<usize> = documented.iter().map(|prefix| position(prefix)).collect();
    assert!(
        observed.windows(2).all(|pair| pair[0] < pair[1]),
        "checks must run in the documented order {documented:?}: {observed:?}"
    );
    assert!(
        ports
            .store
            .has_call_starting_with("is_present /snapshot.git/objects/info/alternates"),
        "{:?}",
        ports.store.calls()
    );
}

#[test]
fn history_reading_stops_at_the_first_commit_without_a_channel() {
    let path = Utf8Path::new("/snapshot.git");
    let mut ports = Ports::accepting();
    // A plain-text toolchain file names no TOML channel, and Dylint's walk
    // fails there rather than reading past it.
    ports.repo.commits = Ok(vec![SELECTED.to_owned(), OLDER.to_owned()]);
    ports
        .repo
        .add_file(SELECTED, "rust-toolchain", "nightly-2026-05-28\n");

    let error = verify_with(path, &manifest(), &ports.support())
        .expect_err("history cannot select a revision");

    assert!(
        matches!(error, ClippySnapshotError::InsufficientHistory { .. }),
        "{error}"
    );
    let reads: Vec<String> = ports
        .repo
        .calls()
        .into_iter()
        .filter(|call| call.starts_with("file_at_commit "))
        .collect();
    assert_eq!(
        reads,
        vec![format!("file_at_commit {path} {SELECTED} rust-toolchain")],
        "the second file and every older commit must be left unread"
    );
}

#[test]
fn the_selected_revision_channel_is_reread_after_selection() {
    let path = Utf8Path::new("/snapshot.git");
    let ports = Ports::accepting();

    verify_with(path, &manifest(), &ports.support()).expect("a consistent snapshot verifies");

    let selected_reads = ports
        .repo
        .calls()
        .into_iter()
        .filter(|call| *call == format!("file_at_commit {path} {SELECTED} rust-toolchain"))
        .count();
    assert_eq!(
        selected_reads, 2,
        "once during the walk, once to check the selected channel"
    );
}

#[test]
fn a_digest_mismatch_is_rejected_before_any_other_call() {
    let root = Utf8Path::new("/root");
    let mut ports = Ports::accepting();
    ports.store.digest = Ok(digest('b'));

    let error = install_with(&artefact(), &manifest(), root, &ports.support())
        .expect_err("a corrupt bundle must not install");

    assert!(
        matches!(error, ClippySnapshotError::ChecksumMismatch { .. }),
        "{error}"
    );
    assert_eq!(
        ports.store.calls(),
        vec!["digest snapshot.bundle".to_owned()]
    );
    assert!(
        ports.bundles.calls().is_empty() && ports.repo.calls().is_empty(),
        "no other observation may run: bundles {:?}, repository {:?}",
        ports.bundles.calls(),
        ports.repo.calls()
    );
}

#[test]
fn unexpected_bundle_heads_are_rejected_before_the_root_is_locked() {
    let root = Utf8Path::new("/root");
    let mut ports = Ports::accepting();
    ports.bundles.heads = Ok(format!("{TIP} refs/tags/v1"));

    let error = install_with(&artefact(), &manifest(), root, &ports.support())
        .expect_err("an unexpected bundle ref must not install");

    assert!(
        matches!(error, ClippySnapshotError::Invalid { .. }),
        "{error}"
    );
    assert!(
        !ports.store.has_call_starting_with("lock ")
            && !ports.store.has_call_starting_with("create_root "),
        "the root must not be locked or created: {:?}",
        ports.store.calls()
    );
    assert_eq!(
        ports.bundles.calls(),
        vec![
            "verify_header snapshot.bundle".to_owned(),
            "list_heads snapshot.bundle".to_owned(),
        ]
    );
}

#[test]
fn a_failed_verification_writes_nothing_and_publishes_nothing() {
    let root = Utf8Path::new("/root");
    let mut ports = Ports::accepting();
    ports.repo.structure = Ok(StructureEvidence {
        is_bare: "false".to_owned(),
        head: String::new(),
        refs: String::new(),
    });

    install_with(&artefact(), &manifest(), root, &ports.support())
        .expect_err("a staged repository that fails verification is not published");

    assert!(
        ports.metadata.calls().is_empty(),
        "metadata must not be written: {:?}",
        ports.metadata.calls()
    );
    assert!(
        !ports.store.has_call_starting_with("publish "),
        "{:?}",
        ports.store.calls()
    );
}

#[test]
fn metadata_is_written_before_the_generation_is_published() {
    let root = Utf8Path::new("/root");
    let ports = Ports::accepting();

    let generation = install_with(&artefact(), &manifest(), root, &ports.support())
        .expect("a consistent bundle installs");

    assert_eq!(generation, root.join(TIP));
    // Derive the staged path rather than spelling it, so the assertion holds
    // on platforms whose separator is not `/`.
    let staged = root.join(".incoming-fake").join("snapshot.git");
    assert_eq!(
        ports.metadata.calls(),
        vec![format!("write {staged}")],
        "metadata belongs inside the staged repository, before the rename"
    );
    let calls = ports.store.calls();
    let write = calls
        .iter()
        .position(|call| call.starts_with("publish "))
        .expect("the staged repository is published");
    assert_eq!(calls[write], format!("publish {staged} {generation}"));
}

#[test]
fn an_existing_generation_is_reused_without_staging_or_publishing() {
    let root = Utf8Path::new("/root");
    let generation = root.join(TIP);
    let mut ports = Ports::accepting();
    ports.store = FakeStore::new(digest('a')).present(&generation);

    let installed = install_with(&artefact(), &manifest(), root, &ports.support())
        .expect("the existing generation verifies");

    assert_eq!(installed, generation);
    assert!(
        !ports.store.has_call_starting_with("staging ")
            && !ports.store.has_call_starting_with("publish ")
            && !ports.metadata.has_call_starting_with("write "),
        "reuse must not stage, publish, or rewrite metadata: store {:?}, metadata {:?}",
        ports.store.calls(),
        ports.metadata.calls()
    );
}

#[test]
fn an_existing_generation_with_wrong_metadata_is_rejected_not_repaired() {
    let root = Utf8Path::new("/root");
    let generation = root.join(TIP);
    let mut ports = Ports::accepting();
    ports.store = FakeStore::new(digest('a')).present(&generation);
    ports.metadata.read = Ok(GenerationMetadata {
        commit: OTHER.to_owned(),
        ..GenerationMetadata::for_manifest(&manifest(), digest('a').as_str())
    });

    let error = install_with(&artefact(), &manifest(), root, &ports.support())
        .expect_err("mismatched metadata must not be repaired in place");

    assert!(
        matches!(&error, ClippySnapshotError::Invalid { check, .. }
            if check.contains("records commit")),
        "{error}"
    );
    assert!(
        !ports.store.has_call_starting_with("publish ")
            && !ports.metadata.has_call_starting_with("write ")
            && ports.repo.calls().is_empty(),
        "a rejected generation is not published, rewritten, or re-observed: store {:?}, metadata \
         {:?}, repository {:?}",
        ports.store.calls(),
        ports.metadata.calls(),
        ports.repo.calls()
    );
}

#[test]
fn reuse_still_verifies_the_existing_repository() {
    let root = Utf8Path::new("/root");
    let generation = root.join(TIP);
    let mut ports = Ports::accepting();
    ports.store = FakeStore::new(digest('a')).present(&generation);
    ports.repo.structure = Ok(StructureEvidence {
        is_bare: "false".to_owned(),
        head: String::new(),
        refs: String::new(),
    });

    let error = install_with(&artefact(), &manifest(), root, &ports.support())
        .expect_err("metadata agreement alone is not verification");

    assert!(
        matches!(&error, ClippySnapshotError::Invalid { check, .. }
            if check == "it is not a bare repository"),
        "{error}"
    );
    assert!(!ports.store.has_call_starting_with("publish "));
}

#[rstest]
#[case::another_commit(OTHER)]
#[case::an_empty_commit("")]
fn the_warm_check_rejects_metadata_for_another_commit(#[case] recorded: &str) {
    let path = Utf8Path::new("/root/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let mut ports = Ports::accepting();
    ports.metadata.read = Ok(GenerationMetadata {
        commit: recorded.to_owned(),
        ..GenerationMetadata::for_manifest(&manifest(), digest('a').as_str())
    });

    let error = check_installed_with(path, TIP, &ports.support())
        .expect_err("stale metadata must not pass the warm check");

    assert!(
        matches!(&error, ClippySnapshotError::Invalid { check, .. }
            if check.contains("its metadata records commit")),
        "{error}"
    );
    assert!(
        ports.repo.calls().is_empty(),
        "refs are only read once the metadata names the pinned commit"
    );
}
