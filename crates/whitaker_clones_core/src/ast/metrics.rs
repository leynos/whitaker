//! Bounded operational metrics for AST lowering.

use std::{sync::Once, time::Duration};

use super::{AstError, AstResult, NormalizedTree};

pub(crate) const LOWER_SPAN_DURATION_SECONDS: &str =
    "whitaker_clones.ast.lower_span.duration_seconds";
pub(crate) const LOWER_SPAN_TOTAL: &str = "whitaker_clones.ast.lower_span.total";
pub(crate) const LOWER_SPAN_PARSER_RECOVERY_TOTAL: &str =
    "whitaker_clones.ast.lower_span.parser_recovery.total";

pub(crate) const OUTCOME_SUCCESS: &str = "success";
pub(crate) const OUTCOME_INVALID_SPAN: &str = "invalid_span";
pub(crate) const OUTCOME_UNPARSABLE_SPAN: &str = "unparsable_span";
pub(crate) const OUTCOME_NODE_BUDGET_EXHAUSTED: &str = "node_budget_exhausted";
pub(crate) const OUTCOME_DEPTH_BUDGET_EXHAUSTED: &str = "depth_budget_exhausted";
pub(crate) const OUTCOME_PARSER_UNAVAILABLE: &str = "parser_unavailable";

static DESCRIBE_METRICS: Once = Once::new();

pub(crate) fn outcome_label(result: &AstResult<NormalizedTree>) -> &'static str {
    match result {
        Ok(_) => OUTCOME_SUCCESS,
        Err(AstError::TreeTooLarge { .. }) => OUTCOME_NODE_BUDGET_EXHAUSTED,
        Err(AstError::TreeTooDeep { .. }) => OUTCOME_DEPTH_BUDGET_EXHAUSTED,
        Err(AstError::UnparsableSpan { .. }) => OUTCOME_UNPARSABLE_SPAN,
        Err(
            AstError::InvalidSpan { .. }
            | AstError::EmptySpan { .. }
            | AstError::NonCharBoundary { .. }
            | AstError::OffsetTooLarge(_)
            | AstError::SpanOutOfBounds { .. },
        ) => OUTCOME_INVALID_SPAN,
        Err(AstError::ParserUnavailable) => OUTCOME_PARSER_UNAVAILABLE,
    }
}

pub(crate) fn record_lower_span_metrics(
    result: &AstResult<NormalizedTree>,
    duration: Duration,
    recovered: bool,
) {
    describe_metrics();
    metrics::histogram!(LOWER_SPAN_DURATION_SECONDS).record(duration.as_secs_f64());
    metrics::counter!(LOWER_SPAN_TOTAL, "outcome" => outcome_label(result)).increment(1);
    if recovered {
        metrics::counter!(LOWER_SPAN_PARSER_RECOVERY_TOTAL).increment(1);
    }
}

fn describe_metrics() {
    DESCRIBE_METRICS.call_once(|| {
        metrics::describe_histogram!(
            LOWER_SPAN_DURATION_SECONDS,
            metrics::Unit::Seconds,
            "Elapsed time spent validating, parsing, and lowering an AST span"
        );
        metrics::describe_counter!(
            LOWER_SPAN_TOTAL,
            "AST span lowering attempts categorized by bounded outcome"
        );
        metrics::describe_counter!(
            LOWER_SPAN_PARSER_RECOVERY_TOTAL,
            "AST span lowering attempts whose source required parser recovery"
        );
    });
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "parser"))]
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};
    use rstest::rstest;

    use super::*;
    use crate::ast::{ByteSpan, KindId, NormalizedNode};

    fn successful_result() -> AstResult<NormalizedTree> {
        let span = ByteSpan::new("x", 0, 1)?;
        Ok(NormalizedTree::new(
            NormalizedNode::new(KindId::new(0), None, Vec::new()),
            span,
        ))
    }

    #[rstest]
    #[case::success(successful_result(), OUTCOME_SUCCESS)]
    #[case::invalid_span(
        Err(AstError::InvalidSpan { start: 2, end: 1 }),
        OUTCOME_INVALID_SPAN
    )]
    #[case::empty_span(Err(AstError::EmptySpan { offset: 1 }), OUTCOME_INVALID_SPAN)]
    #[case::non_char_boundary(
        Err(AstError::NonCharBoundary { offset: 1 }),
        OUTCOME_INVALID_SPAN
    )]
    #[case::offset_too_large(Err(AstError::OffsetTooLarge(usize::MAX)), OUTCOME_INVALID_SPAN)]
    #[case::span_out_of_bounds(
        Err(AstError::SpanOutOfBounds { start: 0, end: 2, len: 1 }),
        OUTCOME_INVALID_SPAN
    )]
    #[case::parser_unavailable(Err(AstError::ParserUnavailable), OUTCOME_PARSER_UNAVAILABLE)]
    #[case::node_budget(
        Err(AstError::TreeTooLarge { limit: 10 }),
        OUTCOME_NODE_BUDGET_EXHAUSTED
    )]
    #[case::depth_budget(
        Err(AstError::TreeTooDeep { limit: 10 }),
        OUTCOME_DEPTH_BUDGET_EXHAUSTED
    )]
    #[case::unparsable_span(
        Err(AstError::UnparsableSpan { start: 0, end: 1 }),
        OUTCOME_UNPARSABLE_SPAN
    )]
    fn ast_results_have_bounded_outcomes(
        #[case] result: AstResult<NormalizedTree>,
        #[case] expected: &'static str,
    ) {
        assert_eq!(outcome_label(&result), expected);
    }

    #[cfg(not(feature = "parser"))]
    #[rstest]
    fn parser_unavailable_stub_records_outcome_and_latency() -> Result<(), AstError> {
        let span = ByteSpan::new("x", 0, 1)?;
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let result = metrics::with_local_recorder(&recorder, || crate::lower_span("x", span));
        let snapshot = snapshotter.snapshot().into_vec();

        assert_eq!(result, Err(AstError::ParserUnavailable));
        assert!(snapshot.iter().any(|(key, _, _, value)| {
            key.key().name() == LOWER_SPAN_TOTAL
                && key.key().labels().any(|label| {
                    label.key() == "outcome" && label.value() == OUTCOME_PARSER_UNAVAILABLE
                })
                && matches!(value, DebugValue::Counter(1))
        }));
        assert!(snapshot.iter().any(|(key, _, _, value)| {
            key.key().name() == LOWER_SPAN_DURATION_SECONDS
                && matches!(value, DebugValue::Histogram(samples) if samples.len() == 1)
        }));
        Ok(())
    }
}
