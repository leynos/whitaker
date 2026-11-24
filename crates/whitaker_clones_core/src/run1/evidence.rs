//! Lowering helpers and conclusive-evidence assembly for Run 1.
//!
//! Lowering happens once per physical fragment: the memo in the pass stores
//! the outcome by physical location, and these helpers turn one fragment into
//! the handful of facts the classifier needs. Failure disposition is the
//! caller's concern, so the mapping lives in `tables::lowering_disposition`.

use super::{
    cosine::CosineMicros,
    error::{Run1Error, Run1Result},
    metrics,
    outcome::AstEvidence,
    tables::{AstVerdict, Disposition, lowering_disposition},
};
use crate::{
    AstError,
    AstHash,
    AstResult,
    ByteSpan,
    NormalizedTree,
    ProductionMultiset,
    TokenFragment,
    ast::{canonical_hash, kind_counts, production_multiset},
    pair_result::node_count,
};

/// Everything the classifier needs to know about one lowered fragment.
pub(super) struct FragmentFacts {
    pub(super) hash: AstHash,
    pub(super) productions: ProductionMultiset,
    pub(super) nodes: usize,
    pub(super) covering_len: usize,
}

/// Evidence for one physical pair: the measured values and the table verdict.
pub(super) struct PairEvidence {
    pub(super) evidence: AstEvidence,
    pub(super) verdict: AstVerdict,
}

/// Build the cosine of two identical trees.
pub(super) fn whole_cosine() -> Run1Result<CosineMicros> {
    CosineMicros::from_micros(1_000_000).ok_or_else(|| Run1Error::InvalidScore {
        value: "1.000000".to_owned(),
    })
}

/// Lower one fragment, mapping failures to their disposition.
pub(super) fn lower_fragment<F>(
    fragment: &TokenFragment,
    lower: &F,
) -> Run1Result<Option<NormalizedTree>>
where
    F: Fn(&str, ByteSpan) -> AstResult<NormalizedTree>,
{
    let span = fragment_span(fragment)?;
    match lower(fragment.source_text(), span) {
        Ok(tree) => {
            metrics::record_feature_vector();
            Ok(Some(tree))
        }
        Err(error) => match lowering_disposition(&error) {
            Disposition::AbortPass => Err(abort_error(fragment, error)),
            Disposition::Inconclusive => Ok(None),
        },
    }
}

/// Convert a fragment's recorded span into the parser's validated span.
fn fragment_span(fragment: &TokenFragment) -> Run1Result<ByteSpan> {
    let span = fragment.span();
    let invalid = |error: AstError| Run1Error::InvalidFragmentSpan {
        fragment_id: fragment.id().as_str().to_owned(),
        source: error,
    };
    let start =
        u32::try_from(span.start).map_err(|_| invalid(AstError::OffsetTooLarge(span.start)))?;
    let end = u32::try_from(span.end).map_err(|_| invalid(AstError::OffsetTooLarge(span.end)))?;
    ByteSpan::new(fragment.source_text(), start, end).map_err(invalid)
}

/// Map an aborting lowering failure onto the pass-level error.
fn abort_error(fragment: &TokenFragment, error: AstError) -> Run1Error {
    match error {
        AstError::ParserUnavailable => Run1Error::ParserUnavailable,
        other => Run1Error::InvalidFragmentSpan {
            fragment_id: fragment.id().as_str().to_owned(),
            source: other,
        },
    }
}

/// Summarize a lowered tree into the facts the classifier needs.
///
/// The tree itself is dropped after this call, so the histogram and production
/// multiset are cloned out of it rather than borrowed.
pub(super) fn summarize(tree: &NormalizedTree) -> FragmentFacts {
    let covering = tree.covering_span();
    let covering_len =
        usize::try_from(covering.end().saturating_sub(covering.start())).unwrap_or(usize::MAX);
    FragmentFacts {
        hash: canonical_hash(tree),
        productions: production_multiset(tree),
        nodes: node_count(&kind_counts(tree)),
        covering_len,
    }
}
