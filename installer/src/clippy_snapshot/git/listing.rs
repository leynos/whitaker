//! Translate Git listings into the values policy compares.
//!
//! Git answers several questions with a listing rather than a value: refs,
//! remotes, and toolchain commits. Parsing those lines is the adapter's job, so
//! policy receives names and object IDs rather than another tool's output
//! format.

use super::super::policy::RefEntry;

/// Split a ref listing into the values policy compares.
///
/// `git for-each-ref` prints one `name object` pair per line. A line without a
/// separator cannot name a ref, so it is dropped here rather than handed to
/// policy as an unreadable value.
pub(super) fn parse_refs(listing: &str) -> Vec<RefEntry> {
    listing
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(name, object)| RefEntry {
            name: name.to_owned(),
            object: object.to_owned(),
        })
        .collect()
}

/// Split a one-name-per-line listing into its entries.
pub(super) fn listing_lines(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
