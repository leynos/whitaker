//! The refinement pass's public result, its classification vocabulary and the
//! reporting call.
//!
//! Split from `refine` so the pass keeps only its algorithm. The pass is a
//! query: it returns a [`Refinement`] and records nothing, and
//! [`record_refinement_metrics`] is the explicit step that turns its counts
//! into metrics at whatever boundary chooses to report them.

use super::{
    metrics,
    outcome::{PairClassification, RefinedPair},
};

/// A refinement pass's results and the counts its caller reports.
///
/// The pass records nothing itself: operational counts come back as data so the
/// query has no hidden side effect, and the caller decides where to report
/// them; a command that composes the pass calls
/// [`record_refinement_metrics`] once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refinement {
    pub(super) pairs: Vec<RefinedPair>,
    pub(super) classifications: Vec<PairClassification>,
    pub(super) lowered_fragments: usize,
}

impl Refinement {
    /// Returns the refined results, ordered by physical pair and token rule.
    #[must_use]
    pub fn pairs(&self) -> &[RefinedPair] { &self.pairs }

    /// Returns one classification per physical pair, in pair order.
    ///
    /// The unit is the *physical pair*, not the result: a pair accepted under
    /// both normalization profiles refines to two results and one entry here.
    #[must_use]
    pub fn classifications(&self) -> &[PairClassification] { &self.classifications }

    /// Returns how many fragments were lowered into a tree.
    ///
    /// Each physical fragment is lowered at most once, so this is also the
    /// number of feature vectors the pass built.
    #[must_use]
    pub const fn lowered_fragment_count(&self) -> usize { self.lowered_fragments }

    /// Consumes the refinement, returning just its results.
    #[must_use]
    pub fn into_pairs(self) -> Vec<RefinedPair> { self.pairs }
}

/// Record a refinement's operational counts.
///
/// The pass records nothing itself, so this is the one call that turns its
/// counts into metrics. An orchestration boundary — the clone-detector command
/// or a test — calls it once per pass, which keeps the query free of hidden
/// side effects and makes the reporting visible at the call site.
pub fn record_refinement_metrics(refinement: &Refinement) {
    metrics::record_refinement(refinement);
}

/// One physical pair's refined results, with the classification they earn.
///
/// An empty outcome means the pair contributed neither a result nor a metric
/// event, which happens only when a group has no entries at all.
#[derive(Default)]
pub(super) struct PairOutcome {
    pub(super) pairs: Vec<RefinedPair>,
    pub(super) classification: Option<PairClassification>,
}
