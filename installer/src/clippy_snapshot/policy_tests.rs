//! The acceptance rules decide from values alone.
//!
//! Every case here is a plain value: a string the adapter would have read, a
//! boolean a probe would have returned, or a hand-worked history. None of
//! them needs Git, a filesystem, or a subprocess, which is the point of the
//! policy boundary.

use rstest::rstest;

use super::*;
use crate::clippy_snapshot::selection::ToolchainChange;

/// A structure observation whose symbolic HEAD names the snapshot branch.
fn structure(is_bare: bool, refs: &str) -> StructureEvidence {
    StructureEvidence {
        is_bare: if is_bare { "true" } else { "false" }.to_owned(),
        head: main_ref(),
        refs: refs.to_owned(),
    }
}

/// The acceptance observation for a healthy repository at `commit`.
fn healthy_structure(commit: PinnedCommit<'_>) -> StructureEvidence {
    structure(true, &format!("{main} {commit}", main = main_ref()))
}

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn accepts_a_bare_repository_with_only_main() {
    assert_eq!(
        accept_structure(
            &healthy_structure(PinnedCommit::new(COMMIT)),
            PinnedCommit::new(COMMIT)
        ),
        Ok(())
    );
}

/// The exact rejection message the policy must produce.
struct ExpectedMessage(&'static str);

#[rstest]
#[case::not_bare(structure(false, ""), ExpectedMessage("it is not a bare repository"))]
#[case::head_on_another_branch(
    StructureEvidence {
        is_bare: "true".to_owned(),
        head: "refs/heads/master".to_owned(),
        refs: String::new(),
    },
    ExpectedMessage("HEAD is `refs/heads/master`, not a symbolic ref to refs/heads/main")
)]
#[case::main_at_another_object(
    structure(true, "refs/heads/main deadbeef"),
    ExpectedMessage(
        "refs/heads/main is deadbeef, expected the pinned commit \
         0123456789abcdef0123456789abcdef01234567"
    )
)]
#[case::no_main(structure(true, ""), ExpectedMessage("it has no refs/heads/main"))]
fn rejects_the_wrong_repository_shape(
    #[case] evidence: StructureEvidence,
    #[case] reason: ExpectedMessage,
) {
    assert_eq!(
        accept_structure(&evidence, PinnedCommit::new(COMMIT)),
        Err(reason.0.to_owned())
    );
}

#[test]
fn reports_extra_refs_before_comparing_the_pinned_object() {
    // The extra ref is the more serious fault, so it must win even though the
    // `main` line also points at the wrong object.
    let evidence = structure(true, "refs/heads/main deadbeef\nrefs/tags/v1 deadbeef");
    assert_eq!(
        accept_structure(&evidence, PinnedCommit::new(COMMIT)),
        Err("it carries refs other than refs/heads/main: refs/tags/v1".to_owned())
    );
}

#[test]
fn ignores_lines_without_a_ref_and_object() {
    // Only `name object` lines carry evidence; a stray blank line or a line
    // with no object must not be mistaken for a ref.
    let evidence = structure(
        true,
        &format!("\nrefs/heads/main-not-a-ref\n{} {COMMIT}", main_ref()),
    );
    assert_eq!(
        accept_structure(&evidence, PinnedCommit::new(COMMIT)),
        Ok(())
    );
}

fn contained(is_shallow: bool, remotes: &[&str]) -> SelfContainedEvidence {
    SelfContainedEvidence {
        is_shallow: if is_shallow { "true" } else { "false" }.to_owned(),
        remotes: remotes.join("\n"),
    }
}

#[test]
fn accepts_complete_history_without_alternates_or_remotes() {
    assert_eq!(accept_self_contained(&contained(false, &[]), false), Ok(()));
}

#[rstest]
#[case::shallow(contained(true, &[]), false, ExpectedMessage("its history is shallow"))]
#[case::alternates(
    contained(false, &[]),
    true,
    ExpectedMessage("it borrows objects through objects/info/alternates")
)]
#[case::one_remote(
    contained(false, &["origin"]),
    false,
    ExpectedMessage("it has remotes (origin), so it could fetch")
)]
#[case::several_remotes(
    contained(false, &["origin", "upstream"]),
    false,
    ExpectedMessage("it has remotes (origin, upstream), so it could fetch")
)]
fn rejects_a_repository_that_could_reach_outside(
    #[case] evidence: SelfContainedEvidence,
    #[case] has_alternates: bool,
    #[case] reason: ExpectedMessage,
) {
    assert_eq!(
        accept_self_contained(&evidence, has_alternates),
        Err(reason.0.to_owned())
    );
}

#[test]
fn reports_shallow_history_before_alternates() {
    assert_eq!(
        accept_self_contained(&contained(true, &["origin"]), true),
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
    assert_eq!(
        accept_bundle_heads(&listing, PinnedCommit::new(COMMIT)),
        Ok(())
    );
}

#[test]
fn accepts_a_bundle_whose_heads_are_in_any_order() {
    let listing = heads(&[
        &format!("{COMMIT} HEAD"),
        &format!("{COMMIT} {}", main_ref()),
    ]);
    assert_eq!(
        accept_bundle_heads(&listing, PinnedCommit::new(COMMIT)),
        Ok(())
    );
}

#[test]
fn rejects_unexpected_bundle_lines_before_a_missing_main() {
    let listing = heads(&[&format!("{COMMIT} refs/tags/v1")]);
    assert_eq!(
        accept_bundle_heads(&listing, PinnedCommit::new(COMMIT)),
        Err(format!(
            "the bundle must carry only refs/heads/main and HEAD at {COMMIT}, but lists: {COMMIT} \
             refs/tags/v1"
        ))
    );
}

#[rstest]
#[case::no_main(
    heads(&[&format!("{COMMIT} HEAD")]),
    ExpectedMessage(
        "the bundle does not carry refs/heads/main at the pinned commit \
         0123456789abcdef0123456789abcdef01234567"
    )
)]
#[case::main_at_another_commit(
    heads(&["deadbeef refs/heads/main"]),
    ExpectedMessage(
        "the bundle must carry only refs/heads/main and HEAD at \
         0123456789abcdef0123456789abcdef01234567, but lists: deadbeef refs/heads/main"
    )
)]
fn rejects_a_bundle_that_does_not_carry_the_pinned_main(
    #[case] listing: String,
    #[case] reason: ExpectedMessage,
) {
    assert_eq!(
        accept_bundle_heads(&listing, PinnedCommit::new(COMMIT)),
        Err(reason.0.to_owned())
    );
}

#[test]
fn accepts_metadata_recording_the_pinned_commit() {
    assert_eq!(accept_recorded_commit(COMMIT, COMMIT), Ok(()));
}

#[rstest]
#[case::another_commit("deadbeef")]
#[case::empty_recorded("")]
fn rejects_metadata_recording_another_commit(#[case] recorded: &str) {
    assert_eq!(
        accept_recorded_commit(recorded, COMMIT),
        Err(format!(
            "its metadata records commit {recorded}, expected {COMMIT}"
        ))
    );
}

#[rstest]
#[case::equal("abc", "abc", true)]
#[case::different("abc", "xyz", false)]
#[case::empty_expected("", "abc", false)]
fn digests_match_only_when_equal(
    #[case] expected: &str,
    #[case] actual: &str,
    #[case] agree: bool,
) {
    assert_eq!(digests_match(expected, actual), agree);
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
        accept_selection(
            &history,
            &SelectionExpectation {
                toolchain: "n-04",
                revision: "b",
                channel: Some("n-04"),
            }
        ),
        Ok(())
    );
}

#[test]
fn rejects_history_that_cannot_support_a_selection() {
    // n-04 is the oldest channel and no older, different channel follows it,
    // so Dylint never yields its introducing commit.
    let history = changes(&[("b", "n-06"), ("a", "n-04")]);
    assert_eq!(
        accept_selection(
            &history,
            &SelectionExpectation {
                toolchain: "n-05",
                revision: "b",
                channel: Some("n-06"),
            }
        ),
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
        accept_selection(
            &history,
            &SelectionExpectation {
                toolchain: "n-04",
                revision: "a",
                channel: Some("n-04"),
            }
        ),
        Err(SelectionRejection::Revision {
            selected: "b".to_owned(),
            expected: "a".to_owned(),
        })
    );
}

#[rstest]
#[case::no_channel(false)]
#[case::another_channel(true)]
fn rejects_a_selected_revision_naming_another_channel(#[case] names_another_channel: bool) {
    let channel = names_another_channel.then_some("n-03");
    let history = changes(&[("b", "n-04"), ("a", "n-03")]);
    assert_eq!(
        accept_selection(
            &history,
            &SelectionExpectation {
                toolchain: "n-04",
                revision: "b",
                channel,
            }
        ),
        Err(SelectionRejection::Channel {
            selected: "b".to_owned(),
            channel: channel.map(ToOwned::to_owned),
            toolchain: "n-04".to_owned(),
        })
    );
}

#[path = "channel_policy_tests.rs"]
mod channels;
