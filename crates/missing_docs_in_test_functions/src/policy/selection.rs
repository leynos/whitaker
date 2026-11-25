//! The candidate-selection predicate.
//!
//! The predicate is deliberately a chain of guard clauses in the fixed gate
//! order, while the exhaustive tests express the same specification as a
//! conjunction plus an ordered scan. The two shapes fail differently under a
//! transcription mistake, which is what makes the paired enumeration worth
//! running.

use super::{
    context::FunctionContext,
    facts::{
        BodyPresence,
        CandidateFacts,
        Documentation,
        HarnessStatus,
        PathSelection,
        Provenance,
        SkipReason,
        Verdict,
    },
};

/// Decides whether one candidate function should be diagnosed.
///
/// Gates are checked in the order harness, provenance, context, body, path,
/// documentation. The first failing gate names the [`SkipReason`], so a skip
/// always reports the earliest reason the contract gives.
///
/// # Examples
///
/// Only an undocumented source function at module level, in a test-harness
/// compilation, is diagnosed:
///
/// ```
/// # #![cfg_attr(feature = "dylint-driver", feature(rustc_private))]
/// # #[cfg(feature = "dylint-driver")] extern crate rustc_driver;
/// use missing_docs_in_test_functions::policy::{
///     BodyPresence,
///     CandidateFacts,
///     Documentation,
///     FunctionContext,
///     HarnessStatus,
///     PathSelection,
///     Provenance,
///     SkipReason,
///     Verdict,
///     select,
/// };
///
/// let candidate = CandidateFacts {
///     harness: HarnessStatus::TestHarness,
///     provenance: Provenance::Source,
///     context: FunctionContext::Module,
///     body: BodyPresence::Present,
///     path: PathSelection::Selected,
///     documentation: Documentation::Undocumented,
/// };
///
/// assert_eq!(
///     select(candidate),
///     Verdict::Diagnose,
///     "an undocumented module function in a test harness is diagnosed"
/// );
///
/// let documented = CandidateFacts {
///     documentation: Documentation::Documented,
///     ..candidate
/// };
/// assert_eq!(
///     select(documented),
///     Verdict::Skip(SkipReason::Documented),
///     "a documented function is skipped at the documentation gate"
/// );
/// ```
#[must_use]
pub fn select(facts: CandidateFacts) -> Verdict {
    if facts.harness == HarnessStatus::NotTestHarness {
        return Verdict::Skip(SkipReason::NotTestHarness);
    }
    if facts.provenance == Provenance::Expansion {
        return Verdict::Skip(SkipReason::NotSourceAuthored);
    }
    if !is_in_scope_context(facts.context) {
        return Verdict::Skip(SkipReason::OutOfScopeContext);
    }
    if facts.body == BodyPresence::Absent {
        return Verdict::Skip(SkipReason::Bodyless);
    }
    if facts.path == PathSelection::NotSelected {
        return Verdict::Skip(SkipReason::PathNotSelected);
    }
    if facts.documentation == Documentation::Documented {
        return Verdict::Skip(SkipReason::Documented);
    }
    Verdict::Diagnose
}

/// Reports whether a function context is in scope for the lint.
///
/// A predicate rather than an `if` chain keeps the three in-scope variants in
/// one place, so adding a context cannot leave the two writes out of step.
#[must_use]
const fn is_in_scope_context(context: FunctionContext) -> bool {
    matches!(
        context,
        FunctionContext::Module | FunctionContext::InherentImpl | FunctionContext::Trait
    )
}
