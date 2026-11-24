//! Typed errors for AST-pass refinement and SARIF Run 1 emission.

use thiserror::Error;

use super::ThresholdKind;
use crate::{
    AstError,
    run0::{Run0Error, SimilarityThreshold},
};

/// Result type for AST-pass refinement and Run 1 emission.
pub type Run1Result<T> = Result<T, Run1Error>;

/// Errors raised while refining token-pass results into SARIF Run 1.
///
/// The variants separate caller mistakes (misordered or non-decimal
/// thresholds, invalid spans) from per-pair evidence gaps, because the first
/// abort the pass while the second only make one pair's evidence inconclusive.
#[derive(Debug, Error)]
pub enum Run1Error {
    /// The configured thresholds are not strictly ordered.
    #[error("similarity threshold `{higher}` must exceed `{lower}`")]
    ThresholdOrder {
        /// The threshold that should be the larger of the pair.
        higher: ThresholdKind,
        /// The threshold that should be the smaller of the pair.
        lower: ThresholdKind,
    },

    /// The Type-3 threshold is not representable with six decimals.
    #[error(
        "type3 threshold {numerator}/{denominator} is not a six-decimal value, so the printed \
         cosine could contradict the decision"
    )]
    ThresholdNotDecimal {
        /// Threshold numerator.
        numerator: usize,
        /// Threshold denominator.
        denominator: usize,
    },

    /// A fragment's recorded span does not describe its source text.
    #[error("fragment `{fragment_id}` has an invalid span: {source}")]
    InvalidFragmentSpan {
        /// Identifier of the fragment whose span was rejected.
        fragment_id: String,
        /// The span validation failure.
        #[source]
        source: AstError,
    },

    /// The parser feature is absent, so no fragment can be lowered.
    #[error("AST refinement requires the `parser` feature")]
    ParserUnavailable,

    /// A computed cosine score is inconsistent with its own definition.
    #[error("cosine score `{value}` is invalid")]
    InvalidScore {
        /// The inconsistent intermediate value.
        value: String,
    },

    /// A threshold was outside `(0, 1]`.
    #[error("similarity threshold `{name}` must satisfy 0 < numerator <= denominator")]
    InvalidThreshold {
        /// Human-readable threshold name.
        name: String,
    },

    /// The token pass rejected the refinement input.
    #[error(transparent)]
    TokenPass(#[from] Run0Error),

    /// SARIF model construction failed.
    #[error(transparent)]
    Sarif(#[from] whitaker_sarif::SarifError),
}

impl Run1Error {
    /// Build a not-decimal error for a rejected Type-3 threshold.
    #[must_use]
    pub const fn not_decimal(threshold: SimilarityThreshold) -> Self {
        Self::ThresholdNotDecimal {
            numerator: threshold.numerator(),
            denominator: threshold.denominator(),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Error-rendering tests that pin the operator-facing messages.

    use rstest::rstest;

    use super::*;

    /// Check that a misordered threshold reports both threshold names.
    #[test]
    fn threshold_order_names_both_thresholds() {
        let error = Run1Error::ThresholdOrder {
            higher: ThresholdKind::Type3,
            lower: ThresholdKind::Type2,
        };
        assert_eq!(
            error.to_string(),
            "similarity threshold `type3` must exceed `type2`",
            "the message must name the misordered pair"
        );
    }

    /// Check that a non-decimal threshold error names the offending ratio and
    /// explains why the ratio is refused.
    #[rstest]
    #[case(1, 3)]
    #[case(2, 3)]
    fn threshold_not_decimal_names_the_ratio(#[case] numerator: usize, #[case] denominator: usize) {
        let threshold = SimilarityThreshold::new_unchecked("type3", numerator, denominator);
        let message = Run1Error::not_decimal(threshold).to_string();
        assert!(
            message.starts_with(&format!("type3 threshold {numerator}/{denominator}")),
            "the message must name the offending ratio, got {message}"
        );
        assert!(
            message.contains("six-decimal"),
            "the message must explain the six-decimal restriction, got {message}"
        );
    }

    /// Check that an invalid fragment span keeps the underlying AST error as
    /// its source, so a caller can still inspect the span failure.
    #[test]
    fn invalid_fragment_span_preserves_the_cause() {
        let error = Run1Error::InvalidFragmentSpan {
            fragment_id: "alpha".to_owned(),
            source: AstError::EmptySpan { offset: 3 },
        };
        let source = std::error::Error::source(&error);
        assert_eq!(
            source.map(ToString::to_string).as_deref(),
            Some("byte span is empty at offset 3"),
            "the AST error must remain reachable as the source"
        );
    }
}
