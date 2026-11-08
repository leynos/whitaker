//! Translate Git listings into the values policy compares.
//!
//! Git answers several questions with a listing rather than a value: refs,
//! remotes, and toolchain commits. Parsing those lines is the adapter's job, so
//! policy receives names and object IDs rather than another tool's output
//! format.

use super::super::policy::{BundleHead, RefEntry};

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

/// Split a bundle head listing into the values policy compares.
///
/// `git bundle list-heads` prints the object first and the ref second, the
/// reverse of `for-each-ref`, so the fields swap here. A line without a
/// separator names no head. Dropping such a line would let the rest of the
/// listing pass as complete evidence, so it is reported instead: the bundle's
/// own header is what policy must judge, and it must see the whole of it.
pub(super) fn parse_bundle_heads(listing: &str) -> Result<Vec<BundleHead>, String> {
    listing
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (object, name) = line.split_once(' ').ok_or_else(|| {
                format!("the bundle head listing has a line without a ref: `{line}`")
            })?;
            Ok(BundleHead {
                object: object.to_owned(),
                name: name.to_owned(),
            })
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

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
