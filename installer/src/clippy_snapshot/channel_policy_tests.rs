//! Channel precedence falls through only on proven absence.
//!
//! Rustup picks the first toolchain file that *exists*, so a read that fails
//! tells the policy nothing about which file would win. These cases keep that
//! distinction between "absent" and "unreadable" explicit.

use super::super::{ChannelRead, ClippySnapshotError, first_channel};

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
fn every_failed_read_surfaces_the_first_failure() {
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
            path: "first".to_owned(),
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
fn a_failed_read_stops_the_walk_before_a_later_present_file() {
    // The failed file may exist and outrank the later one, so its channel
    // cannot be inferred from a file rustup might never reach.
    let reads = [
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
    assert_eq!(first_channel(Vec::new()), Ok(None));
}
