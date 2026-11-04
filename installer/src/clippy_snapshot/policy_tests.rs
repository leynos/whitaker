//! The acceptance rules decide from values alone.
//!
//! Every case here is a plain value: a string the adapter would have read, a
//! boolean a probe would have returned, or a hand-worked history. None of
//! them needs Git, a filesystem, or a subprocess, which is the point of the
//! policy boundary.

use rstest::rstest;

use super::*;
use crate::clippy_snapshot::selection::ToolchainChange;

/// A structure observation with every field accepted unless overridden.
fn structure(is_bare: &str, head: &str, refs: &str) -> StructureEvidence {
    StructureEvidence {
        is_bare: is_bare.to_owned(),
        head: head.to_owned(),
        refs: refs.to_owned(),
    }
}

/// The acceptance observation for a healthy repository at `commit`.
fn healthy_structure(commit: &str) -> StructureEvidence {
    structure(
        "true",
        &main_ref(),
        &format!("{main} {commit}", main = main_ref()),
    )
}

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn accepts_a_bare_repository_with_only_main() {
    assert_eq!(accept_structure(&healthy_structure(COMMIT), COMMIT), Ok(()));
}

#[rstest]
#[case::not_bare(structure("false", &main_ref(), ""), "it is not a bare repository")]
#[case::head_on_another_branch(
    structure("true", "refs/heads/master", ""),
    "HEAD is `refs/heads/master`, not a symbolic ref to refs/heads/main"
)]
fn rejects_the_wrong_repository_shape(#[case] evidence: StructureEvidence, #[case] reason: &str) {
    assert_eq!(accept_structure(&evidence, COMMIT), Err(reason.to_owned()));
}

#[test]
fn reports_extra_refs_before_comparing_the_pinned_object() {
    // The extra ref is the more serious fault, so it must win even though the
    // `main` line also points at the wrong object.
    let evidence = structure(
        "true",
        &main_ref(),
        "refs/heads/main deadbeef\nrefs/tags/v1 deadbeef",
    );
    assert_eq!(
        accept_structure(&evidence, COMMIT),
        Err("it carries refs other than refs/heads/main: refs/tags/v1".to_owned())
    );
}

#[test]
fn rejects_main_at_another_object() {
    let evidence = structure("true", &main_ref(), "refs/heads/main deadbeef");
    assert_eq!(
        accept_structure(&evidence, COMMIT),
        Err(format!(
            "refs/heads/main is deadbeef, expected the pinned commit {COMMIT}"
        ))
    );
}

#[test]
fn rejects_a_listing_without_main() {
    let evidence = structure("true", &main_ref(), "");
    assert_eq!(
        accept_structure(&evidence, COMMIT),
        Err("it has no refs/heads/main".to_owned())
    );
}

#[test]
fn ignores_lines_without_a_ref_and_object() {
    // Only `name object` lines carry evidence; a stray blank line or a line
    // with no object must not be mistaken for a ref.
    let evidence = structure(
        "true",
        &main_ref(),
        &format!("\nrefs/heads/main-not-a-ref\n{} {COMMIT}", main_ref()),
    );
    assert_eq!(accept_structure(&evidence, COMMIT), Ok(()));
}

fn contained(is_shallow: &str, remotes: &str) -> SelfContainedEvidence {
    SelfContainedEvidence {
        is_shallow: is_shallow.to_owned(),
        remotes: remotes.to_owned(),
    }
}

#[test]
fn accepts_complete_history_without_alternates_or_remotes() {
    assert_eq!(
        accept_self_contained(&contained("false", ""), false),
        Ok(())
    );
}

#[rstest]
#[case::shallow(contained("true", ""), false, "its history is shallow")]
#[case::alternates(
    contained("false", ""),
    true,
    "it borrows objects through objects/info/alternates"
)]
#[case::one_remote(
    contained("false", "origin"),
    false,
    "it has remotes (origin), so it could fetch"
)]
#[case::several_remotes(
    contained("false", "origin\nupstream"),
    false,
    "it has remotes (origin, upstream), so it could fetch"
)]
fn rejects_a_repository_that_could_reach_outside(
    #[case] evidence: SelfContainedEvidence,
    #[case] has_alternates: bool,
    #[case] reason: &str,
) {
    assert_eq!(
        accept_self_contained(&evidence, has_alternates),
        Err(reason.to_owned())
    );
}

#[test]
fn reports_shallow_history_before_alternates() {
    assert_eq!(
        accept_self_contained(&contained("true", "origin"), true),
        Err("its history is shallow".to_owned())
    );
}

#[test]
fn accepts_a_clean_object_database() {
    assert_eq!(
        accept_integrity(&IntegrityEvidence {
            succeeded: true,
            stderr: String::new(),
        }),
        Ok(())
    );
}

#[test]
fn reports_the_trimmed_fsck_failure() {
    let evidence = IntegrityEvidence {
        succeeded: false,
        stderr: "  error: object missing\n\n".to_owned(),
    };
    assert_eq!(
        accept_integrity(&evidence),
        Err("git fsck --full failed: error: object missing".to_owned())
    );
}

fn heads(lines: &[&str]) -> String { lines.join("\n") }

#[test]
fn accepts_a_bundle_carrying_only_main_and_head() {
    let listing = heads(&[
        &format!("{COMMIT} {}", main_ref()),
        &format!("{COMMIT} HEAD"),
    ]);
    assert_eq!(accept_bundle_heads(&listing, COMMIT), Ok(()));
}

#[test]
fn accepts_a_bundle_whose_heads_are_in_any_order() {
    let listing = heads(&[
        &format!("{COMMIT} HEAD"),
        &format!("{COMMIT} {}", main_ref()),
    ]);
    assert_eq!(accept_bundle_heads(&listing, COMMIT), Ok(()));
}

#[test]
fn rejects_unexpected_bundle_lines_before_a_missing_main() {
    let listing = heads(&[&format!("{COMMIT} refs/tags/v1")]);
    assert_eq!(
        accept_bundle_heads(&listing, COMMIT),
        Err(format!(
            "the bundle must carry only refs/heads/main and HEAD at {COMMIT}, but lists: {COMMIT} \
             refs/tags/v1"
        ))
    );
}

#[test]
fn rejects_a_bundle_without_main() {
    let listing = heads(&[&format!("{COMMIT} HEAD")]);
    assert_eq!(
        accept_bundle_heads(&listing, COMMIT),
        Err(format!(
            "the bundle does not carry refs/heads/main at the pinned commit {COMMIT}"
        ))
    );
}

#[test]
fn rejects_a_bundle_whose_main_is_at_another_commit() {
    let listing = heads(&["deadbeef refs/heads/main"]);
    assert_eq!(
        accept_bundle_heads(&listing, COMMIT),
        Err("the bundle must carry only refs/heads/main and HEAD at \
             0123456789abcdef0123456789abcdef01234567, but lists: deadbeef refs/heads/main"
            .to_owned())
    );
}

#[rstest]
#[case::equal("abc", "abc", true)]
#[case::different("abc", "xyz", false)]
#[case::one_empty("", "abc", false)]
fn digests_agree_exactly(#[case] expected: &str, #[case] actual: &str, #[case] matches: bool) {
    assert_eq!(digests_match(expected, actual), matches);
}

/// A newest-first history of `(commit, channel)` pairs.
fn changes(pairs: &[(&str, &str)]) -> Vec<ToolchainChange> {
    pairs
        .iter()
        .map(|(commit, channel)| ToolchainChange {
            commit: (*commit).to_owned(),
            channel: (*channel).to_owned(),
        })
        .collect()
}

#[test]
fn accepts_a_selection_that_names_the_pinned_channel() {
    let history = changes(&[("b", "n-04"), ("a", "n-03")]);
    assert_eq!(
        accept_selection(&history, "n-04", "b", Some("n-04")),
        Ok(())
    );
}

#[test]
fn rejects_history_that_cannot_support_a_selection() {
    // n-04 is the oldest channel and no older, different channel follows it,
    // so Dylint never yields its introducing commit.
    let history = changes(&[("b", "n-06"), ("a", "n-04")]);
    assert_eq!(
        accept_selection(&history, "n-05", "b", Some("n-06")),
        Err(SelectionRejection::Insufficient(
            SelectionGap::MissingIntroduction {
                oldest_commit: "a".to_owned(),
                channel: "n-04".to_owned(),
            }
        ))
    );
}

#[test]
fn rejects_a_selection_that_disagrees_with_the_manifest() {
    let history = changes(&[("b", "n-04"), ("a", "n-03")]);
    assert_eq!(
        accept_selection(&history, "n-04", "a", Some("n-04")),
        Err(SelectionRejection::Revision {
            selected: "b".to_owned(),
            expected: "a".to_owned(),
        })
    );
}

#[rstest]
#[case::no_channel(None)]
#[case::another_channel(Some("n-03"))]
fn rejects_a_selected_revision_naming_another_channel(#[case] channel: Option<&str>) {
    let history = changes(&[("b", "n-04"), ("a", "n-03")]);
    assert_eq!(
        accept_selection(&history, "n-04", "b", channel),
        Err(SelectionRejection::Channel {
            selected: "b".to_owned(),
            channel: channel.map(ToOwned::to_owned),
            toolchain: "n-04".to_owned(),
        })
    );
}

#[test]
fn a_present_file_wins_even_when_it_names_no_channel() {
    let reads = [
        Ok(ChannelRead::Present(None)),
        Ok(ChannelRead::Present(Some("n-04".to_owned()))),
    ];
    assert_eq!(first_channel(reads), Ok(None));
}

#[test]
fn an_absent_file_falls_through_to_the_next() {
    let reads = [
        Ok(ChannelRead::Absent),
        Ok(ChannelRead::Present(Some("n-04".to_owned()))),
    ];
    assert_eq!(first_channel(reads), Ok(Some("n-04".to_owned())));
}

#[test]
fn every_absent_file_means_no_channel() {
    let reads = [Ok(ChannelRead::Absent), Ok(ChannelRead::Absent)];
    assert_eq!(first_channel(reads), Ok(None));
}

#[test]
fn every_failed_read_surfaces_the_last_failure() {
    let reads = [
        Err(ClippySnapshotError::NotInstalled {
            path: "first".to_owned(),
        }),
        Err(ClippySnapshotError::NotInstalled {
            path: "second".to_owned(),
        }),
    ];
    assert_eq!(
        first_channel(reads),
        Err(ClippySnapshotError::NotInstalled {
            path: "second".to_owned(),
        })
    );
}

#[test]
fn an_absent_file_does_not_hide_an_earlier_failure() {
    // "Absent" and "unreadable" are different facts; only a successful read
    // may clear a recorded infrastructure failure.
    let reads = [
        Err(ClippySnapshotError::NotInstalled {
            path: "first".to_owned(),
        }),
        Ok(ChannelRead::Absent),
    ];
    assert_eq!(
        first_channel(reads),
        Err(ClippySnapshotError::NotInstalled {
            path: "first".to_owned(),
        })
    );
}

#[test]
fn a_present_file_clears_an_earlier_failure() {
    // The earlier file being unreadable does not matter once a later file
    // decides the channel, exactly as rustup's precedence requires.
    let reads = [
        Err(ClippySnapshotError::NotInstalled {
            path: "first".to_owned(),
        }),
        Ok(ChannelRead::Present(Some("n-04".to_owned()))),
    ];
    assert_eq!(first_channel(reads), Ok(Some("n-04".to_owned())));
}

#[test]
fn an_empty_read_sequence_means_no_channel() {
    assert_eq!(first_channel(Vec::new()), Ok(None));
}
