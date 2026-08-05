//! Operational-metrics tests for Rust syntax lowering.

use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};
use rstest::rstest;

use super::{MAX_AST_DEPTH, MAX_AST_NODES, lower_span};
use crate::ast::metrics::{
    LOWER_SPAN_DURATION_SECONDS, LOWER_SPAN_PARSER_RECOVERY_TOTAL, LOWER_SPAN_TOTAL,
    OUTCOME_DEPTH_BUDGET_EXHAUSTED, OUTCOME_INVALID_SPAN, OUTCOME_NODE_BUDGET_EXHAUSTED,
    OUTCOME_SUCCESS, OUTCOME_UNPARSABLE_SPAN,
};
use crate::{AstError, ByteSpan};

enum MetricsScenario {
    Success,
    InvalidSpan,
    UnparsableSpan,
    NodeBudgetExhausted,
    DepthBudgetExhausted,
}

#[rstest]
#[case::success(MetricsScenario::Success, OUTCOME_SUCCESS, false)]
#[case::invalid_span(MetricsScenario::InvalidSpan, OUTCOME_INVALID_SPAN, false)]
#[case::unparsable_span(MetricsScenario::UnparsableSpan, OUTCOME_UNPARSABLE_SPAN, true)]
#[case::node_budget_exhausted(
    MetricsScenario::NodeBudgetExhausted,
    OUTCOME_NODE_BUDGET_EXHAUSTED,
    false
)]
#[case::depth_budget_exhausted(
    MetricsScenario::DepthBudgetExhausted,
    OUTCOME_DEPTH_BUDGET_EXHAUSTED,
    false
)]
fn lower_span_records_bounded_outcome_and_latency(
    #[case] scenario: MetricsScenario,
    #[case] expected_outcome: &'static str,
    #[case] recovered: bool,
) -> Result<(), AstError> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || run_metrics_scenario(scenario))?;

    assert_lower_span_metrics(snapshotter.snapshot(), expected_outcome, recovered);
    Ok(())
}

#[rstest]
fn parser_recovery_is_recorded_alongside_success() -> Result<(), AstError> {
    let source = "fn valid() {} trailing";
    let span = ByteSpan::new(source, 0, "fn valid() {}".len() as u32)?;
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let result = metrics::with_local_recorder(&recorder, || lower_span(source, span));

    assert!(result.is_ok(), "the selected valid function should lower");
    assert_lower_span_metrics(snapshotter.snapshot(), OUTCOME_SUCCESS, true);
    Ok(())
}

fn run_metrics_scenario(scenario: MetricsScenario) -> Result<(), AstError> {
    let (source, span) = metrics_scenario_input(scenario)?;
    let _result = lower_span(&source, span);
    Ok(())
}

fn metrics_scenario_input(scenario: MetricsScenario) -> Result<(String, ByteSpan), AstError> {
    match scenario {
        MetricsScenario::Success => source_and_whole_span("fn f() {}".to_owned()),
        MetricsScenario::InvalidSpan => {
            let span = ByteSpan::new("longer", 0, 6)?;
            Ok(("short".to_owned(), span))
        }
        MetricsScenario::UnparsableSpan => source_and_whole_span("@error@".to_owned()),
        MetricsScenario::NodeBudgetExhausted => {
            let statements = (0..=MAX_AST_NODES)
                .map(|index| format!("let value_{index} = {index};"))
                .collect::<Vec<_>>()
                .join(" ");
            source_and_whole_span(format!("fn generated() {{ {statements} }}"))
        }
        MetricsScenario::DepthBudgetExhausted => {
            let source = format!(
                "fn deeply_nested() {{ {}0;{} }}",
                "{".repeat(MAX_AST_DEPTH + 1),
                "}".repeat(MAX_AST_DEPTH + 1)
            );
            source_and_whole_span(source)
        }
    }
}

fn source_and_whole_span(source: String) -> Result<(String, ByteSpan), AstError> {
    let span = ByteSpan::new(&source, 0, source.len() as u32)?;
    Ok((source, span))
}

fn assert_lower_span_metrics(snapshot: Snapshot, expected_outcome: &str, recovered: bool) {
    let snapshot = snapshot.into_vec();
    let has_outcome = snapshot.iter().any(|(key, _, _, value)| {
        key.key().name() == LOWER_SPAN_TOTAL
            && key
                .key()
                .labels()
                .any(|label| label.key() == "outcome" && label.value() == expected_outcome)
            && matches!(value, DebugValue::Counter(1))
    });
    let has_latency = snapshot.iter().any(|(key, _, _, value)| {
        key.key().name() == LOWER_SPAN_DURATION_SECONDS
            && matches!(value, DebugValue::Histogram(samples) if samples.len() == 1)
    });
    let has_recovery = snapshot.iter().any(|(key, _, _, value)| {
        key.key().name() == LOWER_SPAN_PARSER_RECOVERY_TOTAL
            && matches!(value, DebugValue::Counter(1))
    });

    assert!(
        has_outcome,
        "expected outcome {expected_outcome}; got {snapshot:#?}"
    );
    assert!(
        has_latency,
        "expected one latency sample; got {snapshot:#?}"
    );
    assert_eq!(
        has_recovery, recovered,
        "unexpected recovery metric: {snapshot:#?}"
    );
}
