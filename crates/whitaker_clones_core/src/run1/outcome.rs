//! Refinement outcomes produced by the AST pass.

use crate::{AcceptedPair, CandidatePair, CloneRule, CosineMicros, SimilarityRatio, ast::AstHash};

/// Conclusive structural evidence for one physical fragment pair.
///
/// The cosine is meaningful for both sides because equal hashes imply a cosine
/// of exactly one; when the hashes differ it is the measured near-miss score.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{
///     AstEvidence,
///     CosineMicros,
///     ast::{ByteSpan, KindId, NormalizedNode, NormalizedTree},
///     canonical_hash,
/// };
///
/// let span = ByteSpan::new("fn f() {}", 0, 2)?;
/// let tree = NormalizedTree::new(
///     NormalizedNode::new(KindId::new(1), None, Vec::new()),
///     span,
///     span,
/// );
/// let hash = canonical_hash(&tree);
/// let score = CosineMicros::from_micros(850_000).expect("valid score");
/// let evidence = AstEvidence::new(hash, hash, score);
///
/// assert!(evidence.hashes_equal());
/// assert_eq!(evidence.cosine().get(), 850_000);
/// # Ok::<(), whitaker_clones_core::AstError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AstEvidence {
    left_hash: AstHash,
    right_hash: AstHash,
    cosine: CosineMicros,
}

impl AstEvidence {
    /// Creates conclusive evidence from both canonical hashes and the cosine.
    #[must_use]
    pub const fn new(left_hash: AstHash, right_hash: AstHash, cosine: CosineMicros) -> Self {
        Self {
            left_hash,
            right_hash,
            cosine,
        }
    }

    /// Returns the primary side's canonical AST hash.
    #[must_use]
    pub const fn left_hash(&self) -> AstHash { self.left_hash }

    /// Returns the related side's canonical AST hash.
    #[must_use]
    pub const fn right_hash(&self) -> AstHash { self.right_hash }

    /// Returns the floored cosine similarity of the two histograms.
    #[must_use]
    pub const fn cosine(&self) -> CosineMicros { self.cosine }

    /// Reports whether both canonical trees hash equal.
    #[must_use]
    pub fn hashes_equal(&self) -> bool { self.left_hash == self.right_hash }
}

/// How one physical pair was classified by the refinement pass.
///
/// This is the pass's own vocabulary. The metric module maps each variant to a
/// bounded label, so the labels stay a telemetry concern and the classification
/// stays a domain one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairClassification {
    /// A token-pass result was carried forward unchanged.
    Carried,
    /// A token-pass result was reclassified as a near-miss.
    Reclassified,
    /// The AST pass added a result.
    Added,
    /// The pair was conclusive but neither table reported it.
    Rejected,
    /// The evidence was inconclusive, so the token verdict stands.
    Inconclusive,
}

/// One Run 1 result, before emission.
///
/// The three variants mirror the three provenance kinds: a token-pass result
/// carried forward, one reclassified under `WHK003`, or a result the AST pass
/// added.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{
///     AcceptedPair,
///     CandidatePair,
///     CloneRule,
///     FragmentId,
///     NormProfile,
///     RefinedPair,
///     SimilarityRatio,
/// };
///
/// let pair = CandidatePair::new(FragmentId::from("a"), FragmentId::from("b"))
///     .expect("distinct fragments");
/// let accepted = AcceptedPair::new(pair, NormProfile::T2, SimilarityRatio::new(1, 1));
/// let refined = RefinedPair::Carried {
///     token: accepted,
///     ast: None,
/// };
///
/// assert_eq!(refined.rule(), CloneRule::Type2);
/// assert_eq!(refined.token_rule(), Some(CloneRule::Type2));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefinedPair {
    /// A token-pass result carried forward unchanged.
    Carried {
        /// The accepted token-pass pair.
        token: AcceptedPair,
        /// Conclusive AST evidence, absent when the evidence was inconclusive.
        ast: Option<AstEvidence>,
    },
    /// A token-pass result reclassified as a near-miss.
    Reclassified {
        /// The accepted token-pass pair.
        token: AcceptedPair,
        /// The conclusive evidence that justified reclassification.
        ast: AstEvidence,
    },
    /// A near-miss the token pass did not report.
    Added {
        /// The representative candidate pair.
        pair: CandidatePair,
        /// The reported rule.
        rule: CloneRule,
        /// The representative candidate's Jaccard score.
        jaccard: SimilarityRatio,
        /// The conclusive evidence that justified the result.
        ast: AstEvidence,
    },
}

impl RefinedPair {
    /// Returns the rule this result reports.
    #[must_use]
    pub const fn rule(&self) -> CloneRule {
        match self {
            Self::Carried { token, .. } => token.rule(),
            Self::Reclassified { .. } => CloneRule::Type3,
            Self::Added { rule, .. } => *rule,
        }
    }

    /// Returns the conclusive AST evidence, when there is some.
    #[must_use]
    pub const fn ast(&self) -> Option<&AstEvidence> {
        match self {
            Self::Carried { ast, .. } => ast.as_ref(),
            Self::Reclassified { ast, .. } | Self::Added { ast, .. } => Some(ast),
        }
    }

    /// Returns the token-pass rule this result derives from, when it does.
    #[must_use]
    pub const fn token_rule(&self) -> Option<CloneRule> {
        match self {
            Self::Carried { token, .. } | Self::Reclassified { token, .. } => Some(token.rule()),
            Self::Added { .. } => None,
        }
    }

    /// Returns the Jaccard score this result reports.
    #[must_use]
    pub const fn jaccard(&self) -> SimilarityRatio {
        match self {
            Self::Carried { token, .. } | Self::Reclassified { token, .. } => token.score(),
            Self::Added { jaccard, .. } => *jaccard,
        }
    }

    /// Returns the candidate pair this result describes.
    #[must_use]
    pub const fn pair(&self) -> &CandidatePair {
        match self {
            Self::Carried { token, .. } | Self::Reclassified { token, .. } => token.pair(),
            Self::Added { pair, .. } => pair,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Outcome accessor and provenance tests.

    use super::*;
    use crate::{FragmentId, NormProfile};

    /// Build an accepted pair over two identifiers with the given profile.
    fn accepted(left: &str, right: &str, profile: NormProfile) -> AcceptedPair {
        let pair = CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
            .expect("distinct identifiers");
        AcceptedPair::new(pair, profile, SimilarityRatio::new(1, 1))
    }

    /// Build evidence from two distinguishable hashes.
    fn evidence(equal: bool) -> AstEvidence {
        let left = AstHash::from_hex("0123456789abcdef");
        let right = if equal {
            left
        } else {
            AstHash::from_hex("fedcba9876543210")
        };
        AstEvidence::new(
            left,
            right,
            CosineMicros::from_micros(850_000).expect("valid score"),
        )
    }

    /// Check the rule and evidence of a carried result.
    #[test]
    fn carried_result_keeps_its_rule() {
        let refined = RefinedPair::Carried {
            token: accepted("a", "b", NormProfile::T1),
            ast: Some(evidence(true)),
        };

        assert_eq!(
            refined.rule(),
            CloneRule::Type1,
            "a carried result keeps its rule"
        );
        assert!(refined.ast().is_some(), "the evidence must be exposed");
    }

    /// Check the rule and provenance of a reclassified result.
    #[test]
    fn reclassified_result_reports_type3() {
        let refined = RefinedPair::Reclassified {
            token: accepted("a", "b", NormProfile::T2),
            ast: evidence(false),
        };

        assert_eq!(
            refined.rule(),
            CloneRule::Type3,
            "reclassification reports WHK003"
        );
        assert_eq!(
            refined.token_rule(),
            Some(CloneRule::Type2),
            "the token rule must remain available as provenance"
        );
    }

    /// Check the rule and provenance of an added result.
    #[test]
    fn added_result_has_no_token_rule() {
        let pair = CandidatePair::new(FragmentId::from("a"), FragmentId::from("b"))
            .expect("distinct identifiers");
        let refined = RefinedPair::Added {
            pair,
            rule: CloneRule::Type2,
            jaccard: SimilarityRatio::new(9, 10),
            ast: evidence(true),
        };

        assert_eq!(refined.rule(), CloneRule::Type2, "the added rule is kept");
        assert_eq!(
            refined.token_rule(),
            None,
            "an added result has no token-pass rule"
        );
    }

    /// Check that inconclusive evidence is reported as absent.
    #[test]
    fn inconclusive_evidence_is_absent() {
        let refined = RefinedPair::Carried {
            token: accepted("a", "b", NormProfile::T2),
            ast: None,
        };

        assert!(
            refined.ast().is_none(),
            "inconclusive evidence must be absent"
        );
    }
}
