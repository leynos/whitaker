//! Classification tables and lowering-error dispositions.
//!
//! Decision D5 fixes two tables. Table A classifies every accepted token pair,
//! so each Run 0 result has exactly one Run 1 counterpart. Table B classifies a
//! physical pair that no profile accepted. Both tables are pure functions with
//! no wildcard matches, and their domains are enumerated by `ALL` arrays so a
//! new variant breaks the exhaustive tests until a row is added.
//!
//! The pass is positive-evidence only: raising the cosine never removes a
//! reported pair, and the only rule change is from `WHK001` or `WHK002` to
//! `WHK003`.

use crate::{AstError, CloneRule, NormProfile};

/// The structural verdict for one physical fragment pair.
///
/// The four values are the entire domain of decision tables A and B (D5). They
/// are deliberately coarse: the tables depend only on hash equality and on
/// whether the cosine meets the threshold, never on the cosine's exact value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AstVerdict {
    /// The canonical trees are identical, so the pair is rename-equivalent.
    HashesEqual,
    /// The trees differ and the cosine is at least the Type-3 threshold.
    HashesDifferMeetingThreshold,
    /// The trees differ and the cosine is below the Type-3 threshold.
    HashesDifferBelowThreshold,
    /// The evidence is inconclusive, so the token verdict stands.
    Inconclusive,
}

impl AstVerdict {
    /// Every verdict, in a fixed order for exhaustive table tests.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 4] = [
        Self::HashesEqual,
        Self::HashesDifferMeetingThreshold,
        Self::HashesDifferBelowThreshold,
        Self::Inconclusive,
    ];
}

/// The rule a token-pass result reports.
///
/// Run 0 emits only `WHK001` and `WHK002`, so the narrower type keeps the
/// classification tables exhaustive without a `WHK003` arm that could never
/// fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TokenRule {
    /// Type-1, from the `T1` profile.
    Type1,
    /// Type-2, from the `T2` profile.
    Type2,
}

impl TokenRule {
    /// Every token rule, in a fixed order for exhaustive table tests.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 2] = [Self::Type1, Self::Type2];

    /// Returns the clone rule this token rule reports.
    pub(crate) const fn rule(self) -> CloneRule {
        match self {
            Self::Type1 => CloneRule::Type1,
            Self::Type2 => CloneRule::Type2,
        }
    }

    /// Returns the token rule for a normalization profile.
    pub(crate) const fn from_profile(profile: NormProfile) -> Self {
        match profile {
            NormProfile::T1 => Self::Type1,
            NormProfile::T2 => Self::Type2,
        }
    }
}

/// Classify one accepted token-pass pair (decision table A).
///
/// A token result is carried unless the trees differ *and* the cosine reaches
/// the Type-3 threshold; only that combination is positive evidence of a
/// near-miss rather than a rename.
pub(crate) const fn classify_accepted(token: TokenRule, verdict: AstVerdict) -> CloneRule {
    classify_row(token.rule(), verdict)
}

/// Apply table A's row shape to a carried rule.
const fn classify_row(token_rule: CloneRule, verdict: AstVerdict) -> CloneRule {
    match verdict {
        AstVerdict::HashesEqual
        | AstVerdict::HashesDifferBelowThreshold
        | AstVerdict::Inconclusive => token_rule,
        AstVerdict::HashesDifferMeetingThreshold => CloneRule::Type3,
    }
}

/// Classify a physical pair no profile accepted (decision table B).
///
/// Equal hashes mean the code differs only in identifiers, literals and types,
/// which is Type-2 in the clone taxonomy, so the added result reports
/// `WHK002` (open question OQ2, option (a)). Otherwise only a cosine at or
/// above the Type-3 threshold is worth reporting.
pub(crate) const fn classify_unaccepted(verdict: AstVerdict) -> Option<CloneRule> {
    match verdict {
        AstVerdict::HashesEqual => Some(CloneRule::Type2),
        AstVerdict::HashesDifferMeetingThreshold => Some(CloneRule::Type3),
        AstVerdict::HashesDifferBelowThreshold | AstVerdict::Inconclusive => None,
    }
}

/// What a lowering failure means for the pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Disposition {
    /// The failure is the caller's, so the whole pass stops.
    AbortPass,
    /// The failure is confined to one pair, which becomes inconclusive.
    Inconclusive,
}

/// Map every lowering error to its disposition (decision D11).
///
/// The match is exhaustive on purpose: a new [`AstError`] variant must be
/// assigned a disposition before the crate compiles.
pub(crate) const fn lowering_disposition(error: &AstError) -> Disposition {
    match error {
        AstError::ParserUnavailable
        | AstError::InvalidSpan { .. }
        | AstError::EmptySpan { .. }
        | AstError::NonCharBoundary { .. }
        | AstError::SpanOutOfBounds { .. }
        | AstError::OffsetTooLarge(_) => Disposition::AbortPass,
        AstError::UnparsableSpan { .. }
        | AstError::TreeTooLarge { .. }
        | AstError::TreeTooDeep { .. } => Disposition::Inconclusive,
    }
}

#[cfg(test)]
mod tests {
    //! Exhaustive decision-table tests and their monotonicity property.

    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    /// Enumerate table A's domain as `(token rule, verdict)` rows.
    fn table_a_domain() -> Vec<(TokenRule, AstVerdict)> {
        TokenRule::ALL
            .into_iter()
            .flat_map(|token| AstVerdict::ALL.map(|verdict| (token, verdict)))
            .collect()
    }

    /// Check that table A has one row per element of its domain product.
    #[test]
    fn table_a_is_complete() {
        let rows = table_a_domain();

        assert_eq!(
            rows.len(),
            TokenRule::ALL.len() * AstVerdict::ALL.len(),
            "table A must classify every token rule and verdict combination"
        );
        assert_eq!(rows.len(), 8, "table A has eight rows");
    }

    /// Check every row of table A against the decision log.
    #[rstest]
    #[case(TokenRule::Type1, AstVerdict::HashesEqual, CloneRule::Type1)]
    #[case(
        TokenRule::Type1,
        AstVerdict::HashesDifferMeetingThreshold,
        CloneRule::Type3
    )]
    #[case(
        TokenRule::Type1,
        AstVerdict::HashesDifferBelowThreshold,
        CloneRule::Type1
    )]
    #[case(TokenRule::Type1, AstVerdict::Inconclusive, CloneRule::Type1)]
    #[case(TokenRule::Type2, AstVerdict::HashesEqual, CloneRule::Type2)]
    #[case(
        TokenRule::Type2,
        AstVerdict::HashesDifferMeetingThreshold,
        CloneRule::Type3
    )]
    #[case(
        TokenRule::Type2,
        AstVerdict::HashesDifferBelowThreshold,
        CloneRule::Type2
    )]
    #[case(TokenRule::Type2, AstVerdict::Inconclusive, CloneRule::Type2)]
    fn table_a_rows(
        #[case] token: TokenRule,
        #[case] verdict: AstVerdict,
        #[case] expected: CloneRule,
    ) {
        assert_eq!(
            classify_accepted(token, verdict),
            expected,
            "unexpected rule for {token:?} under {verdict:?}"
        );
    }

    /// Check every row of table B against the decision log.
    #[rstest]
    #[case(AstVerdict::HashesEqual, Some(CloneRule::Type2))]
    #[case(AstVerdict::HashesDifferMeetingThreshold, Some(CloneRule::Type3))]
    #[case(AstVerdict::HashesDifferBelowThreshold, None)]
    #[case(AstVerdict::Inconclusive, None)]
    fn table_b_rows(#[case] verdict: AstVerdict, #[case] expected: Option<CloneRule>) {
        assert_eq!(
            classify_unaccepted(verdict),
            expected,
            "unexpected rule for an unaccepted pair under {verdict:?}"
        );
    }

    /// Check that raising the cosine never removes a reported pair and never
    /// weakens a rule.
    #[test]
    fn raising_the_cosine_never_removes_a_result() {
        let below = classify_unaccepted(AstVerdict::HashesDifferBelowThreshold);
        let meeting = classify_unaccepted(AstVerdict::HashesDifferMeetingThreshold);

        assert!(
            below.is_none() && meeting.is_some(),
            "an unaccepted pair reported below the threshold must also be reported above it"
        );
        for token in TokenRule::ALL {
            let low = classify_accepted(token, AstVerdict::HashesDifferBelowThreshold);
            let high = classify_accepted(token, AstVerdict::HashesDifferMeetingThreshold);
            assert!(
                low == token.rule() && high == CloneRule::Type3,
                "raising the cosine must only strengthen {token:?}"
            );
        }
    }

    proptest! {
        /// Check the monotonicity property over generated cosine pairs.
        #[test]
        fn monotonicity_holds_for_generated_scores(
            lower in 0_u32..=1_000_000,
            delta in 0_u32..=1_000_000,
            threshold in 1_u32..=1_000_000,
        ) {
            let higher = lower.saturating_add(delta).min(1_000_000);
            let verdict_at = |micros: u32| {
                if micros >= threshold {
                    AstVerdict::HashesDifferMeetingThreshold
                } else {
                    AstVerdict::HashesDifferBelowThreshold
                }
            };
            let low = classify_unaccepted(verdict_at(lower));
            let high = classify_unaccepted(verdict_at(higher));
            prop_assert!(
                low.is_some() <= high.is_some(),
                "a pair reported at {lower} micros must stay reported at {higher}"
            );
        }
    }

    /// Check that every lowering error has a disposition and that the
    /// aborting variants are exactly the caller errors.
    #[rstest]
    #[case(AstError::ParserUnavailable, Disposition::AbortPass)]
    #[case(AstError::InvalidSpan { start: 2, end: 1 }, Disposition::AbortPass)]
    #[case(AstError::EmptySpan { offset: 1 }, Disposition::AbortPass)]
    #[case(AstError::NonCharBoundary { offset: 1 }, Disposition::AbortPass)]
    #[case(
        AstError::SpanOutOfBounds { start: 0, end: 2, len: 1 },
        Disposition::AbortPass
    )]
    #[case(AstError::OffsetTooLarge(usize::MAX), Disposition::AbortPass)]
    #[case(AstError::UnparsableSpan { start: 0, end: 1 }, Disposition::Inconclusive)]
    #[case(AstError::TreeTooLarge { limit: 10 }, Disposition::Inconclusive)]
    #[case(AstError::TreeTooDeep { limit: 10 }, Disposition::Inconclusive)]
    fn every_error_has_a_disposition(#[case] error: AstError, #[case] expected: Disposition) {
        assert_eq!(
            lowering_disposition(&error),
            expected,
            "unexpected disposition for {error:?}"
        );
    }
}
