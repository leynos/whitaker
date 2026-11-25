//! Refinement of token-pass results into Run 1 outcomes.
//!
//! The pass groups both accepted pairs and candidate pairs by *physical*
//! location, so the T1 and T2 fragments of one code region share a single body
//! of AST evidence. It lowers each physical fragment at most once, applies the
//! decision tables in `tables`, and never removes or weakens a token-pass
//! result.

use std::collections::BTreeMap;

use super::{
    classify::{self},
    config::{AstPassConfig, RefinementInput},
    cosine::CosineScore,
    error::{Run1Error, Run1Result},
    evidence::{FragmentFacts, PairEvidence, lower_fragment, summarize, whole_cosine},
    metrics,
    outcome::{AstEvidence, RefinedPair},
    physical::{self, PhysicalPair},
    tables::{AstVerdict, TokenRule, classify_accepted, classify_unaccepted},
};
use crate::{
    AcceptedPair,
    AstResult,
    ByteSpan,
    CandidatePair,
    FragmentLocation,
    NormProfile,
    NormalizedTree,
    Run0Error,
    SimilarityRatio,
    TokenFragment,
    ast::lower_span,
    pair_result::fragment_map,
    run0::jaccard_similarity,
};

/// Refine the token pass's fragments, candidates and accepted pairs.
///
/// The returned outcomes are ordered by physical pair and, within a pair, by
/// token rule, so two runs over the same input produce identical output.
///
/// # Errors
///
/// Returns [`Run1Error::ThresholdOrder`] or [`Run1Error::ThresholdNotDecimal`]
/// for an unusable threshold configuration, [`Run1Error::ParserUnavailable`]
/// when this build has no parser, [`Run1Error::InvalidFragmentSpan`] for a
/// fragment whose span does not describe its source text, and
/// [`Run1Error::TokenPass`] for missing or empty fragments and mixed profiles.
pub fn refine_candidate_pairs(
    input: &RefinementInput<'_>,
    config: &AstPassConfig,
) -> Run1Result<Vec<RefinedPair>> {
    refine_with_lowering(input, config, lower_span)
}

/// Refine with an injected lowering function.
///
/// Production always passes [`lower_span`]; tests inject synthetic trees and
/// failures so every classification row and every failure path is reachable
/// without contriving real Rust source.
pub(crate) fn refine_with_lowering<F>(
    input: &RefinementInput<'_>,
    config: &AstPassConfig,
    lower: F,
) -> Run1Result<Vec<RefinedPair>>
where
    F: Fn(&str, ByteSpan) -> AstResult<NormalizedTree>,
{
    classify::validate_pass_config(input.token_config(), config)?;

    let mut pass = Pass::new(input, config, lower);
    let accepted = pass.group_accepted()?;
    let candidates = pass.group_candidates()?;

    let mut refined = Vec::new();
    for pair in physical::union_keys(&accepted, &candidates) {
        if let Some(entries) = accepted.get(&pair) {
            refined.extend(pass.refine_accepted(entries)?);
        } else if let Some(entries) = candidates.get(&pair) {
            refined.extend(pass.refine_unaccepted(entries)?);
        }
    }
    Ok(refined)
}

/// State for one refinement pass.
struct Pass<'a, F> {
    accepted: &'a [AcceptedPair],
    candidates: &'a [CandidatePair],
    fragments: &'a [TokenFragment],
    index: BTreeMap<&'a str, &'a TokenFragment>,
    config: &'a AstPassConfig,
    lower: F,
    memo: BTreeMap<FragmentLocation, Option<NormalizedTree>>,
}

impl<'a, F> Pass<'a, F>
where
    F: Fn(&str, ByteSpan) -> AstResult<NormalizedTree>,
{
    /// Prepare a pass over the given input.
    fn new<'b: 'a>(input: &'a RefinementInput<'b>, config: &'a AstPassConfig, lower: F) -> Self {
        let fragments = input.fragments();
        Self {
            accepted: input.accepted(),
            candidates: input.candidates(),
            fragments,
            index: fragment_map(fragments),
            config,
            lower,
            memo: BTreeMap::new(),
        }
    }

    /// Resolve both sides of a candidate pair.
    fn resolve(&self, pair: &CandidatePair) -> Run1Result<(&'a TokenFragment, &'a TokenFragment)> {
        let find = |id: &str| {
            self.index.get(id).copied().ok_or_else(|| {
                Run1Error::TokenPass(Run0Error::MissingFragment {
                    fragment_id: id.to_owned(),
                })
            })
        };
        Ok((find(pair.left().as_str())?, find(pair.right().as_str())?))
    }

    /// Group accepted pairs by physical location, ordered by token rule.
    fn group_accepted(&self) -> Run1Result<BTreeMap<PhysicalPair, Vec<AcceptedPair>>> {
        let mut grouped: BTreeMap<PhysicalPair, Vec<AcceptedPair>> = BTreeMap::new();
        for accepted in self.accepted {
            let (left, right) = self.resolve(accepted.pair())?;
            let key = PhysicalPair::new(left.location(), right.location());
            grouped.entry(key).or_default().push(accepted.clone());
        }
        for entries in grouped.values_mut() {
            entries.sort_by_key(AcceptedPair::rule);
        }
        Ok(grouped)
    }

    /// Group candidate pairs by physical location, rejecting mixed profiles.
    fn group_candidates(&self) -> Run1Result<BTreeMap<PhysicalPair, Vec<CandidatePair>>> {
        let mut grouped: BTreeMap<PhysicalPair, Vec<CandidatePair>> = BTreeMap::new();
        for pair in self.candidates {
            let (left, right) = self.resolve(pair)?;
            if left.profile() != right.profile() {
                return Err(Run1Error::TokenPass(Run0Error::MixedProfiles {
                    left_fragment: left.id().as_str().to_owned(),
                    right_fragment: right.id().as_str().to_owned(),
                }));
            }
            let key = PhysicalPair::new(left.location(), right.location());
            let entries = grouped.entry(key).or_default();
            if !entries.contains(pair) {
                entries.push(pair.clone());
            }
        }
        Ok(grouped)
    }

    /// Refine one physical pair that at least one profile accepted.
    fn refine_accepted(&mut self, entries: &[AcceptedPair]) -> Run1Result<Vec<RefinedPair>> {
        let Some(first) = entries.first() else {
            return Ok(Vec::new());
        };
        let (left, right) = self.resolve(first.pair())?;
        let key = PhysicalPair::new(left.location(), right.location());
        let evidence = self.evidence(&key)?;

        let refined = entries
            .iter()
            .map(|accepted| refined_entry(accepted, evidence.as_ref()))
            .collect::<Vec<_>>();
        if evidence.is_none() {
            metrics::record_pair_classification(metrics::INCONCLUSIVE);
        }
        record_classifications(&refined);
        Ok(refined)
    }

    /// Refine one physical pair that no profile accepted.
    fn refine_unaccepted(&mut self, entries: &[CandidatePair]) -> Run1Result<Vec<RefinedPair>> {
        let Some(representative) = self.representative(entries)? else {
            return Ok(Vec::new());
        };
        let (left, right) = self.resolve(&representative)?;
        let key = PhysicalPair::new(left.location(), right.location());
        let evidence = self.evidence(&key)?;
        let verdict = evidence
            .as_ref()
            .map_or(AstVerdict::Inconclusive, |pair| pair.verdict);

        let Some(rule) = classify_unaccepted(verdict) else {
            metrics::record_pair_classification(rejection_label(evidence.is_some()));
            return Ok(Vec::new());
        };
        let Some(pair) = evidence else {
            return Ok(Vec::new());
        };
        let jaccard = self.jaccard(&representative)?;
        metrics::record_pair_classification(metrics::ADDED);
        Ok(vec![RefinedPair::Added {
            pair: representative,
            rule,
            jaccard,
            ast: pair.evidence,
        }])
    }

    /// Choose the representative candidate: a `T2` candidate if present,
    /// otherwise the least candidate pair.
    fn representative(&self, entries: &[CandidatePair]) -> Run1Result<Option<CandidatePair>> {
        let mut best: Option<(bool, CandidatePair)> = None;
        for pair in entries {
            let (left, _) = self.resolve(pair)?;
            let is_t2 = left.profile() == NormProfile::T2;
            let better = match &best {
                None => true,
                Some((best_is_t2, best_pair)) => {
                    (is_t2 && !*best_is_t2) || (is_t2 == *best_is_t2 && pair < best_pair)
                }
            };
            if better {
                best = Some((is_t2, pair.clone()));
            }
        }
        Ok(best.map(|(_, pair)| pair))
    }

    /// Compute the Jaccard score of a candidate pair.
    ///
    /// # Errors
    ///
    /// Returns [`Run1Error::TokenPass`] when either side retains no
    /// fingerprints, because set-based scoring is then undefined.
    fn jaccard(&self, pair: &CandidatePair) -> Run1Result<SimilarityRatio> {
        let (left, right) = self.resolve(pair)?;
        jaccard_similarity(left.retained_fingerprints(), right.retained_fingerprints()).ok_or_else(
            || {
                let empty = if left.retained_fingerprints().is_empty() {
                    left.id()
                } else {
                    right.id()
                };
                Run1Error::TokenPass(Run0Error::EmptyFingerprintSet {
                    fragment_id: empty.as_str().to_owned(),
                })
            },
        )
    }

    /// Compute conclusive evidence for one physical pair, if there is any.
    fn evidence(&mut self, key: &PhysicalPair) -> Run1Result<Option<PairEvidence>> {
        let Some((left_fragment, left)) = self.lowered_facts(&key.left)? else {
            return Ok(None);
        };
        let Some((right_fragment, right)) = self.lowered_facts(&key.right)? else {
            return Ok(None);
        };
        if !self.has_conclusive_shape(left_fragment, &left)
            || !self.has_conclusive_shape(right_fragment, &right)
        {
            return Ok(None);
        }
        self.measured_evidence(&left, &right)
    }

    /// Lower one side and report its fragment, or `None` when it is missing.
    fn lowered_facts(
        &mut self,
        location: &FragmentLocation,
    ) -> Run1Result<Option<(&'a TokenFragment, FragmentFacts)>> {
        let Some(fragment) = self.fragment_at(location) else {
            return Ok(None);
        };
        Ok(self.facts(location)?.map(|facts| (fragment, facts)))
    }

    /// Turn two conclusive-shape facts into the measured evidence and verdict.
    fn measured_evidence(
        &self,
        left: &FragmentFacts,
        right: &FragmentFacts,
    ) -> Run1Result<Option<PairEvidence>> {
        if left.hash == right.hash {
            return Ok(Some(PairEvidence {
                evidence: AstEvidence::new(left.hash, right.hash, whole_cosine()?),
                verdict: AstVerdict::HashesEqual,
            }));
        }
        let Some(score) = CosineScore::between_productions(&left.productions, &right.productions)
        else {
            return Ok(None);
        };
        let verdict = if score.meets(self.config.type3_threshold()) {
            AstVerdict::HashesDifferMeetingThreshold
        } else {
            AstVerdict::HashesDifferBelowThreshold
        };
        Ok(Some(PairEvidence {
            evidence: AstEvidence::new(left.hash, right.hash, score.micros()?),
            verdict,
        }))
    }

    /// Report whether one fragment has conclusive evidence shape.
    fn has_conclusive_shape(&self, fragment: &TokenFragment, facts: &FragmentFacts) -> bool {
        facts.nodes >= self.config.min_nodes()
            && facts.covering_len
                <= fragment
                    .span()
                    .len()
                    .saturating_mul(self.config.max_cover_expansion())
    }

    /// Summarize one fragment's lowered tree, lowering it at most once.
    fn facts(&mut self, location: &FragmentLocation) -> Run1Result<Option<FragmentFacts>> {
        Ok(self.lower_location(location)?.map(summarize))
    }

    /// Lower one physical location at most once per pass.
    fn lower_location(
        &mut self,
        location: &FragmentLocation,
    ) -> Run1Result<Option<&NormalizedTree>> {
        if !self.memo.contains_key(location) {
            let Some(fragment) = self.fragment_at(location) else {
                return Ok(None);
            };
            let lowered = lower_fragment(fragment, &self.lower)?;
            self.memo.insert(location.clone(), lowered);
        }
        Ok(self.memo.get(location).and_then(Option::as_ref))
    }

    /// Return a token fragment registered at `location`.
    fn fragment_at(&self, location: &FragmentLocation) -> Option<&'a TokenFragment> {
        self.fragments
            .iter()
            .find(|fragment| fragment.location() == location)
    }
}

/// Build one refined outcome for an accepted pair.
///
/// Reclassification happens only when the table returns a stronger rule, and
/// the table can only do that for conclusive evidence, so the two arms that
/// carry evidence are exhaustive over the reachable states.
fn refined_entry(accepted: &AcceptedPair, evidence: Option<&PairEvidence>) -> RefinedPair {
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
const fn rejection_label(has_evidence: bool) -> &'static str {
    if has_evidence {
        metrics::REJECTED
    } else {
        metrics::INCONCLUSIVE
    }
}

/// Record one classification metric per refined outcome.
fn record_classifications(refined: &[RefinedPair]) {
    for pair in refined {
        metrics::record_pair_classification(match pair {
            RefinedPair::Carried { .. } => metrics::CARRIED,
            RefinedPair::Reclassified { .. } => metrics::RECLASSIFIED,
            RefinedPair::Added { .. } => metrics::ADDED,
        });
    }
}

#[cfg(test)]
#[path = "refine_metrics_tests.rs"]
mod metrics_tests;
#[cfg(test)]
#[path = "refine_pair_tests.rs"]
mod pair_tests;
#[cfg(test)]
#[path = "refine_tests.rs"]
mod tests;
