//! Installation stages, verifies, records, and publishes through the ports.

use camino::Utf8Path;
use rstest::rstest;

use super::{
    super::{
        error::ClippySnapshotError,
        fakes::FakeStore,
        generation::GenerationMetadata,
        install::{from_sidecar_with, install_with},
        policy::StructureEvidence,
        verify::check_installed_with,
    },
    support::{OTHER, Ports, TIP, artefact, digest, manifest},
};

#[test]
fn the_sidecar_is_read_through_the_store() {
    let mut ports = Ports::accepting();
    let recorded = digest('b');
    ports.store.text = Ok(format!("{recorded}  snapshot.bundle\n"));

    let artefact = from_sidecar_with(&ports.store, Utf8Path::new("snapshot.bundle"), &manifest())
        .expect("the scripted sidecar names a digest");

    assert_eq!(artefact.sha256, recorded);
    assert_eq!(
        ports.store.calls(),
        vec![
            "is_present_file snapshot.bundle".to_owned(),
            "absolute snapshot.bundle".to_owned(),
            "read_to_string snapshot.bundle.sha256".to_owned(),
        ],
        "the sidecar must be read through the port, not directly from disk"
    );
}

/// A sidecar read failure carries the reason once, not a nested error.
///
/// `Io`'s display already names the path, its remedy, and the no-fallback
/// rule; embedding it in `Sidecar` would repeat that advice and contradict it.
#[test]
fn a_sidecar_read_failure_carries_only_the_reason() {
    let mut ports = Ports::accepting();
    ports.store.text = Err(ClippySnapshotError::Io {
        path: "snapshot.bundle.sha256".to_owned(),
        reason: "permission denied".to_owned(),
    });

    let error = from_sidecar_with(&ports.store, Utf8Path::new("snapshot.bundle"), &manifest())
        .expect_err("an unreadable sidecar is an error");

    let message = error.to_string();
    assert!(
        matches!(&error, ClippySnapshotError::Sidecar { .. }),
        "{message}"
    );
    assert!(message.contains("permission denied"), "{message}");
    assert_eq!(
        message.matches("Whitaker never falls back").count(),
        1,
        "the no-fallback rule must appear once: {message}"
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
