//! Pure helpers that turn refinement outcomes into results and metric labels.
//!
//! Split from `refine` to keep both files inside the module budget. Everything
//! here is a pure function of a fragment index, an accepted pair or a slice of
//! outcomes, so the pass's mutable state stays in one place and these rules can
//! be read without it.

use std::collections::BTreeMap;

use super::{
    evidence::PairEvidence,
    metrics,
    outcome::RefinedPair,
    tables::{TokenRule, classify_accepted},
};
use crate::{AcceptedPair, FragmentLocation, TokenFragment};

/// Index fragments by physical location, keeping the first registered.
///
/// The T1 and T2 fragments of one region share a location, and either can
/// supply the source text, so the first is kept for deterministic behaviour.
pub(super) fn location_index(
    fragments: &[TokenFragment],
) -> BTreeMap<FragmentLocation, &TokenFragment> {
    let mut index = BTreeMap::new();
    for fragment in fragments {
        index.entry(fragment.location().clone()).or_insert(fragment);
    }
    index
}

/// Build one refined outcome for an accepted pair.
///
/// Reclassification happens only when the table returns a stronger rule, and
/// the table can only do that for conclusive evidence, so the two arms that
/// carry evidence are exhaustive over the reachable states.
pub(super) fn refined_entry(
    accepted: &AcceptedPair,
    evidence: Option<&PairEvidence>,
) -> RefinedPair {
    let Some(pair) = evidence else {
        return RefinedPair::Carried {
            token: accepted.clone(),
            ast: None,
        };
    };
    let rule = classify_accepted(TokenRule::from_profile(accepted.profile()), pair.verdict);
    if rule == accepted.rule() {
        return RefinedPair::Carried {
            token: accepted.clone(),
            ast: Some(pair.evidence),
        };
    }
    RefinedPair::Reclassified {
        token: accepted.clone(),
        ast: pair.evidence,
    }
}

/// Return the metric label for a pair that produced no added result.
pub(super) const fn rejection_label(has_evidence: bool) -> &'static str {
    if has_evidence {
        metrics::REJECTED
    } else {
        metrics::INCONCLUSIVE
    }
}

/// Classify one physical pair for the pair counter.
///
/// A pair's outcome is the strongest thing that happened to it: any
/// reclassification makes it `reclassified`, otherwise any evidence makes it
/// `carried`, and no evidence makes it `inconclusive`. The per-result
/// provenance stays on each `RefinedPair`; this label is only for the counter,
/// whose documented unit is the physical pair.
pub(super) fn pair_label(refined: &[RefinedPair], has_evidence: bool) -> &'static str {
    if refined.iter().any(|pair| {
        matches!(
            pair,
            RefinedPair::Reclassified { .. } | RefinedPair::Added { .. }
        )
    }) {
        return metrics::RECLASSIFIED;
    }
    if has_evidence {
        return metrics::CARRIED;
    }
    metrics::INCONCLUSIVE
}
