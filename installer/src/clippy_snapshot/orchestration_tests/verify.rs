//! Verification asks the ports in order and stops at the first rejection.

use camino::Utf8Path;

use super::{
    super::{error::ClippySnapshotError, policy::StructureEvidence, verify::verify_with},
    support::{OLDER, Ports, SELECTED, manifest},
};

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
    // Derive the probed path rather than spelling it, so the assertion holds
    // on platforms whose separator is not `/`.
    let alternates = path.join("objects/info/alternates");
    assert!(
        ports
            .store
            .has_call_starting_with(&format!("is_present {alternates}")),
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
