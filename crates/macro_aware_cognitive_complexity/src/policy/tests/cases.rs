//! Explicit scoring cases covering each contract clause.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::support::{authored, generated};
use crate::policy::{Event, Observed, Score, score};

/// The score of a body with no charged events.
const BASELINE: Score = Score {
    branches: 1,
    returns: 0,
    adjustment: 0,
    total: 1,
};

/// One scoring expectation for the return-adjustment contract.
///
/// The events, the body shape, the expected score, and the reason travel
/// together so each case states its evidence once, and the shared assertion
/// keeps the cases free of copied assertion blocks.
struct Case<'a> {
    /// Charged events in the order the traversal reports them.
    events: &'a [Event],
    /// Whether the scored body returns `Result`.
    returns_result: bool,
    /// The score the policy must produce.
    expected: Score,
    /// Why the contract requires that score.
    reason: &'a str,
}

/// Assert that the policy scores `case` exactly as expected.
fn assert_case(case: &Case<'_>) {
    assert_eq!(
        score(case.events, case.returns_result),
        case.expected,
        "{}",
        case.reason
    );
}

/// Verify that an empty body scores the baseline in either result shape.
#[rstest]
#[case::non_result(false)]
#[case::result(true)]
fn empty_stream_scores_baseline(#[case] returns_result: bool) {
    assert_eq!(
        score(&[], returns_result),
        BASELINE,
        "an empty body is only the baseline"
    );
}

/// Verify that a generated event of any charge category leaves the baseline untouched.
#[rstest]
#[case::branch(Observed::Branch)]
#[case::guard(Observed::Guard)]
#[case::ret(Observed::Return)]
#[case::neutral(Observed::Neutral)]
fn generated_only_events_preserve_baseline(#[case] observed: Observed) {
    let events = [generated(observed)];

    assert_eq!(
        score(&events, false),
        BASELINE,
        "a generated {observed:?} must not charge outside a Result body"
    );
    assert_eq!(
        score(&events, true),
        BASELINE,
        "a generated {observed:?} must not charge inside a Result body"
    );
}

/// Verify that an authored branch or guard raises the total by one.
#[rstest]
#[case::branch(Observed::Branch)]
#[case::guard(Observed::Guard)]
fn authored_branch_or_guard_charges(#[case] observed: Observed) {
    let events = [authored(observed)];

    assert_eq!(
        score(&events, false),
        Score {
            branches: 2,
            returns: 0,
            adjustment: 0,
            total: 2
        },
        "an authored {observed:?} contributes one branch charge"
    );
}

/// Verify that a generated branch followed by an authored neutral keeps the baseline.
#[test]
fn generated_branch_then_authored_neutral_keeps_baseline() {
    let events = [generated(Observed::Branch), authored(Observed::Neutral)];

    assert_eq!(
        score(&events, false),
        BASELINE,
        "a neutral event resets only adjacency and never charges"
    );
}

/// Verify the return-adjustment contract across result shapes and neighbours.
///
/// Each case states the charged events, whether the body returns `Result`, the
/// score the contract requires, and why. Keeping the clauses in one table means
/// a change to return credit or adjacency is reviewed against every case at
/// once, and each case name still identifies the clause it pins.
#[rstest]
#[case::lone_return_outside_result(
    &[authored(Observed::Return)],
    false,
    Score { branches: 1, returns: 1, adjustment: 0, total: 1 },
    "half of one return truncates away outside a Result body"
)]
#[case::lone_return_inside_result(
    &[authored(Observed::Return)],
    true,
    Score { branches: 1, returns: 1, adjustment: 1, total: 0 },
    "a Result body credits a lone return in full"
)]
#[case::consecutive_returns_non_result(
    &[authored(Observed::Return), authored(Observed::Return)],
    false,
    Score { branches: 1, returns: 1, adjustment: 0, total: 1 },
    "only the first of two adjacent returns charges"
)]
#[case::consecutive_returns_result(
    &[authored(Observed::Return), authored(Observed::Return)],
    true,
    Score { branches: 1, returns: 1, adjustment: 1, total: 0 },
    "only the first of two adjacent returns charges"
)]
#[case::authored_neutral_separates_non_result(
    &[
        authored(Observed::Return),
        authored(Observed::Neutral),
        authored(Observed::Return),
    ],
    false,
    Score { branches: 1, returns: 2, adjustment: 1, total: 0 },
    "an authored neutral resets return adjacency"
)]
#[case::authored_neutral_separates_result(
    &[
        authored(Observed::Return),
        authored(Observed::Neutral),
        authored(Observed::Return),
    ],
    true,
    Score { branches: 1, returns: 2, adjustment: 2, total: 1 },
    "an authored neutral resets return adjacency"
)]
#[case::generated_neutral_keeps_adjacency_non_result(
    &[
        authored(Observed::Return),
        generated(Observed::Neutral),
        authored(Observed::Return),
    ],
    false,
    Score { branches: 1, returns: 1, adjustment: 0, total: 1 },
    "an intervening generated event leaves returns adjacent"
)]
#[case::generated_neutral_keeps_adjacency_result(
    &[
        authored(Observed::Return),
        generated(Observed::Neutral),
        authored(Observed::Return),
    ],
    true,
    Score { branches: 1, returns: 1, adjustment: 1, total: 0 },
    "an intervening generated event leaves returns adjacent"
)]
#[case::adjustment_larger_than_branches(
    &[
        authored(Observed::Return),
        authored(Observed::Neutral),
        authored(Observed::Return),
        authored(Observed::Neutral),
        authored(Observed::Return),
    ],
    true,
    Score { branches: 1, returns: 3, adjustment: 3, total: 1 },
    "a total never falls below the branch count when the adjustment is larger"
)]
#[case::branch_and_return_in_result(
    &[authored(Observed::Branch), authored(Observed::Return)],
    true,
    Score { branches: 2, returns: 1, adjustment: 1, total: 1 },
    "one branch charge survives the return adjustment"
)]
#[case::generated_guard_non_result(
    &[
        authored(Observed::Return),
        generated(Observed::Guard),
        authored(Observed::Return),
    ],
    false,
    Score { branches: 1, returns: 1, adjustment: 0, total: 1 },
    "a generated guard is not authored code"
)]
fn score_credits_and_adjusts_returns(
    #[case] events: &[Event],
    #[case] returns_result: bool,
    #[case] expected: Score,
    #[case] reason: &str,
) {
    assert_case(&Case {
        events,
        returns_result,
        expected,
        reason,
    });
}

/// Verify that a total must strictly exceed a limit to report.
#[rstest]
#[case::two_at_two(2, 2, false)]
#[case::two_at_one(2, 1, true)]
#[case::two_at_zero(2, 0, true)]
#[case::one_at_zero(1, 0, true)]
#[case::one_at_one(1, 1, false)]
fn exceeds_is_strict(#[case] total: u64, #[case] limit: u64, #[case] expected: bool) {
    let candidate = Score {
        branches: total,
        returns: 0,
        adjustment: 0,
        total,
    };

    assert_eq!(
        candidate.exceeds(limit),
        expected,
        "total {total} against limit {limit}"
    );
}
