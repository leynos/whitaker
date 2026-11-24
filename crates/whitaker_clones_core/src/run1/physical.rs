//! Canonical physical-pair keys for Run 1 evidence and identity.
//!
//! A fragment's physical location is `(file_uri, span)` and is independent of
//! the normalization profile, so the T1 and T2 fragments of one code region
//! share one body of AST evidence. Ordering the two locations canonically is
//! what lets grouping and memoization key on the pair rather than on the
//! profile-bearing fragment identifiers.

use std::collections::BTreeMap;

use crate::{AcceptedPair, CandidatePair, FragmentLocation};

/// A canonically ordered pair of physical fragment locations.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct PhysicalPair {
    pub(super) left: FragmentLocation,
    pub(super) right: FragmentLocation,
}

impl PhysicalPair {
    /// Order two locations canonically.
    pub(super) fn new(first: &FragmentLocation, second: &FragmentLocation) -> Self {
        if first <= second {
            return Self {
                left: first.clone(),
                right: second.clone(),
            };
        }
        Self {
            left: second.clone(),
            right: first.clone(),
        }
    }
}

/// Return every physical pair named by either grouping, in canonical order.
pub(super) fn union_keys(
    accepted: &BTreeMap<PhysicalPair, Vec<AcceptedPair>>,
    candidates: &BTreeMap<PhysicalPair, Vec<CandidatePair>>,
) -> Vec<PhysicalPair> {
    let mut keys = accepted
        .keys()
        .chain(candidates.keys())
        .cloned()
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    keys
}
