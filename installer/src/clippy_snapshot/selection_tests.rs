//! The selection kernel agrees with Dylint's own walk.
//!
//! Three independent oracles back the kernel: hand-worked histories, a
//! brute-force emulation of Dylint's per-commit walk over generated
//! histories, and `dylint_internal` 6.3.0 itself run against a real Git
//! fixture. An ignored test repeats the last check on the published snapshot.

use super::*;
use crate::clippy_snapshot::test_fixtures::{
    DEFAULT_HISTORY, DEFAULT_SELECTED, FIXTURE_TOOLCHAIN, FixtureRepo, Step, git,
};
use proptest::prelude::*;
use rstest::rstest;

/// Build a newest-first change list from `(commit, channel)` pairs.
fn changes(pairs: &[(&str, &str)]) -> Vec<ToolchainChange> {
    pairs
        .iter()
        .map(|(commit, channel)| ToolchainChange {
            commit: (*commit).to_owned(),
            channel: (*channel).to_owned(),
        })
        .collect()
}

#[rstest]
#[case::exact_match(&[("c", "n-05"), ("b", "n-04"), ("a", "n-03")], "n-04", Some("b"))]
#[case::newest_earlier(&[("c", "n-06"), ("b", "n-03"), ("a", "n-01")], "n-05", Some("b"))]
#[case::merged_run(&[("d", "n-04"), ("c", "n-04"), ("b", "n-02"), ("a", "n-01")], "n-04", Some("c"))]
#[case::tip_is_earlier(&[("b", "n-02"), ("a", "n-01")], "n-09", Some("b"))]
#[case::introduction_missing(&[("b", "n-06"), ("a", "n-04")], "n-05", None)]
#[case::every_channel_later(&[("b", "n-08"), ("a", "n-07")], "n-05", None)]
fn selects_as_dylint_does(
    #[case] pairs: &[(&str, &str)],
    #[case] toolchain: &str,
    #[case] expected: Option<&str>,
) {
    let history = changes(pairs);
    let selected = select_revision(&history, toolchain)
        .ok()
        .map(|c| c.commit.as_str());
    assert_eq!(selected, expected);
}

#[test]
fn reports_why_history_is_insufficient() {
    let history = changes(&[("b", "n-06"), ("a", "n-04")]);
    let gap = select_revision(&history, "n-05").expect_err("no introduction of n-04");
    assert_eq!(
        gap,
        SelectionGap::MissingIntroduction {
            oldest_commit: "a".to_owned(),
            channel: "n-04".to_owned(),
        }
    );
}

/// Dylint's walk over every commit, newest first: emit the oldest commit of
/// each channel run once an older, different channel is seen, and drop the
/// final run when the history ends.
fn dylint_walk(channels: &[u8], toolchain: u8) -> Option<usize> {
    let mut emitted = Vec::new();
    for index in 1..channels.len() {
        if channels[index] != channels[index - 1] {
            emitted.push(index - 1);
        }
    }
    emitted
        .into_iter()
        .find(|&index| channels[index] <= toolchain)
}

/// The kernel's view of the same history: commits whose channel differs from
/// their parent's, plus edits that rewrite the file without changing it, plus
/// the root, where the file was added.
fn kernel_walk(channels: &[u8], rewrites: &[bool], toolchain: u8) -> Option<usize> {
    let last = channels.len() - 1;
    let touched: Vec<usize> = (0..channels.len())
        .filter(|&i| i == last || rewrites[i] || channels[i] != channels[i + 1])
        .collect();
    let history: Vec<ToolchainChange> = touched
        .iter()
        .map(|&i| ToolchainChange {
            commit: i.to_string(),
            channel: format!("nightly-{:03}", channels[i]),
        })
        .collect();
    let toolchain = format!("nightly-{toolchain:03}");
    select_revision(&history, &toolchain)
        .ok()
        .map(|change| change.commit.parse().expect("index"))
}

proptest! {
    /// Over arbitrary histories, the kernel selects the commit Dylint does.
    #[test]
    fn kernel_matches_a_brute_force_dylint_walk(
        (channels, rewrites) in (1usize..40).prop_flat_map(|len| (
            // Any order: a reverted rustup moves the channel backwards.
            prop::collection::vec(0u8..12, len),
            prop::collection::vec(any::<bool>(), len),
        )),
        toolchain in 0u8..14,
    ) {
        prop_assert_eq!(
            kernel_walk(&channels, &rewrites, toolchain),
            dylint_walk(&channels, toolchain)
        );
    }
}

/// Run Dylint 6.3.0's own selection against the repository at `repo`.
///
/// `clippy_repository` caches its clone per thread, so each call gets a fresh
/// thread; the variable is set under the shared environment lock.
fn dylint_selection(repo: &Utf8Path, toolchain: &str) -> Option<String> {
    let repo = repo.to_owned();
    let toolchain = toolchain.to_owned();
    let _guard =
        whitaker_common::test_support::EnvVarGuard::set("DYLINT_CLIPPY_REPO", repo.as_str());
    std::thread::spawn(move || {
        let revs = dylint_internal::clippy_utils::Revs::new(true).expect("open the repository");
        let mut iter = revs.channel_iter().expect("walk the history");
        iter.find(|rev| {
            rev.as_ref()
                .map_or(true, |rev| rev.channel.as_str() <= toolchain.as_str())
        })
        .and_then(Result::ok)
        .map(|rev| rev.oid.to_string())
    })
    .join()
    .expect("the selection thread completes")
}

/// Read and select through the Git layer, as verification does.
fn whitaker_selection(repo: &Utf8Path, head: &str, toolchain: &str) -> Option<String> {
    let changes = toolchain_changes(SnapshotGit::local(), repo, head).expect("read history");
    select_revision(&changes, toolchain)
        .ok()
        .map(|change| change.commit.clone())
}

#[rstest]
#[case::default_history(&DEFAULT_HISTORY, Some(DEFAULT_SELECTED))]
#[case::nightly_reached_by_an_earlier_channel(
    &[Step::Channel("nightly-2026-05-01"), Step::Channel("nightly-2026-05-20"),
      Step::Channel("nightly-2026-06-30")],
    Some(1),
)]
#[case::history_too_short(&[Step::Channel(FIXTURE_TOOLCHAIN), Step::Unrelated], None)]
fn agrees_with_dylint_on_real_git_history(#[case] steps: &[Step], #[case] expected: Option<usize>) {
    let repo = FixtureRepo::new(steps);
    let bare = repo.path("snapshot.git");
    git(
        &repo.work,
        &["clone", "--quiet", "--bare", ".", bare.as_str()],
    );
    let expected = expected.map(|index| repo.commits[index].clone());

    assert_eq!(
        whitaker_selection(&bare, repo.tip(), FIXTURE_TOOLCHAIN),
        expected
    );
    assert_eq!(dylint_selection(&bare, FIXTURE_TOOLCHAIN), expected);
}

/// The published snapshot selects the recorded revision under both
/// implementations. Set `WHITAKER_CLIPPY_SNAPSHOT_REPO` to an installed
/// generation (for example from `make clippy-snapshot`) to run it.
#[test]
#[ignore = "needs an installed Clippy snapshot in WHITAKER_CLIPPY_SNAPSHOT_REPO"]
fn the_published_snapshot_selects_the_recorded_revision() {
    let repo = std::env::var("WHITAKER_CLIPPY_SNAPSHOT_REPO")
        .expect("WHITAKER_CLIPPY_SNAPSHOT_REPO names an installed snapshot");
    let repo = Utf8Path::new(&repo);
    let manifest = crate::clippy_snapshot::embedded_manifest().expect("embedded manifest");
    let expected = Some(manifest.selected_revision.to_string());

    assert_eq!(
        whitaker_selection(repo, manifest.commit.as_str(), &manifest.toolchain),
        expected
    );
    assert_eq!(dylint_selection(repo, &manifest.toolchain), expected);
    crate::clippy_snapshot::verify_repository(repo, manifest).expect("the snapshot verifies");
}
