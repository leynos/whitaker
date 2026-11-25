//! Channel precedence falls through only on proven absence.
//!
//! Rustup picks the first toolchain file that *exists*, so a read that fails
//! tells the policy nothing about which file would win. These cases keep that
//! distinction between "absent" and "unreadable" explicit.

use proptest::prelude::*;

use super::super::{ChannelRead, first_channel};
use crate::clippy_snapshot::ClippySnapshotError;

/// Two reads whose failure type is the boundary error.
type Reads = [Result<ChannelRead, ClippySnapshotError>; 2];

/// One read of any shape: absent, present with or without a channel, or failed.
///
/// Failures carry the file they tried to read, so a property can name the
/// failure it expects rather than accept any error.
fn any_read() -> impl Strategy<Value = Result<ChannelRead, ClippySnapshotError>> {
    prop_oneof![
        Just(Ok(ChannelRead::Absent)),
        Just(Ok(ChannelRead::Present(None))),
        (0u8..4).prop_map(|n| Ok(ChannelRead::Present(Some(format!("nightly-{n}"))))),
        (0u8..8).prop_map(|n| Err(ClippySnapshotError::NotInstalled {
            path: format!("file-{n}"),
        })),
    ]
}

/// Return whether `reads` already contains the answer the walk would report.
fn decides(reads: &[Result<ChannelRead, ClippySnapshotError>]) -> bool {
    reads
        .iter()
        .any(|read| !matches!(read, Ok(ChannelRead::Absent)))
}

#[test]
fn a_present_file_wins_even_when_it_names_no_channel() {
    let reads: Reads = [
        Ok(ChannelRead::Present(None)),
        Ok(ChannelRead::Present(Some("n-04".to_owned()))),
    ];
    assert_eq!(first_channel(reads), Ok(None));
}

#[test]
fn an_absent_file_falls_through_to_the_next() {
    let reads: Reads = [
        Ok(ChannelRead::Absent),
        Ok(ChannelRead::Present(Some("n-04".to_owned()))),
    ];
    assert_eq!(first_channel(reads), Ok(Some("n-04".to_owned())));
}

#[test]
fn every_absent_file_means_no_channel() {
    let reads: Reads = [Ok(ChannelRead::Absent), Ok(ChannelRead::Absent)];
    assert_eq!(first_channel(reads), Ok(None));
}

#[test]
fn every_failed_read_surfaces_the_first_failure() {
    let reads: Reads = [
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
            path: "first".to_owned(),
        })
    );
}

#[test]
fn an_absent_file_does_not_hide_an_earlier_failure() {
    // "Absent" and "unreadable" are different facts; only a successful read
    // may clear a recorded infrastructure failure.
    let reads: Reads = [
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
fn a_failed_read_stops_the_walk_before_a_later_present_file() {
    // The failed file may exist and outrank the later one, so its channel
    // cannot be inferred from a file rustup might never reach.
    let reads: Reads = [
        Err(ClippySnapshotError::NotInstalled {
            path: "first".to_owned(),
        }),
        Ok(ChannelRead::Present(Some("n-04".to_owned()))),
    ];
    assert_eq!(
        first_channel(reads),
        Err(ClippySnapshotError::NotInstalled {
            path: "first".to_owned(),
        })
    );
}

#[test]
fn an_empty_read_sequence_means_no_channel() {
    assert_eq!(first_channel::<ClippySnapshotError>(Vec::new()), Ok(None));
}

proptest! {
    /// Once the walk has found a present file or a failure, it stops there.
    ///
    /// This is the precedence rule itself rather than one example of it: no
    /// amount of later evidence may change an answer already decided.
    #[test]
    fn a_decided_walk_ignores_every_later_read(
        prefix in proptest::collection::vec(any_read(), 1..4),
        suffix in proptest::collection::vec(any_read(), 0..4),
    ) {
        prop_assume!(decides(&prefix));
        let mut reads = prefix.clone();
        reads.extend(suffix);
        prop_assert_eq!(first_channel(reads), first_channel(prefix));
    }

    /// With no failed read, the first present file's channel wins.
    ///
    /// The expectation is computed by searching for the first present file,
    /// not by replaying the walk, so the two disagree if precedence breaks.
    /// The search's outer `Option` says a present file was found and the inner
    /// one carries its channel, if it named one.
    #[test]
    fn the_first_present_file_wins(reads in proptest::collection::vec(any_read(), 0..6)) {
        prop_assume!(reads.iter().all(Result::is_ok));
        let found = reads.iter().find_map(|read| match read {
            Ok(ChannelRead::Present(channel)) => Some(channel.clone()),
            _ => None,
        });
        prop_assert_eq!(first_channel(reads), Ok(found.flatten()));
    }

    /// Absence falls through to the next file; a failure after it stops there.
    #[test]
    fn an_absent_prefix_falls_through_to_the_first_failure(
        prefix in proptest::collection::vec(Just(Ok(ChannelRead::Absent)), 0..4),
        failure in (0u8..8).prop_map(|n| ClippySnapshotError::NotInstalled {
            path: format!("file-{n}"),
        }),
        suffix in proptest::collection::vec(any_read(), 0..4),
    ) {
        let mut reads: Vec<Result<ChannelRead, ClippySnapshotError>> = prefix;
        reads.push(Err(failure.clone()));
        reads.extend(suffix);
        prop_assert_eq!(first_channel(reads), Err(failure));
    }
}
