//! Explicit scoring cases covering each contract clause.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::support::{authored, generated};
use crate::policy::{Observed, Score, score};

/// The score of a body with no charged events.
const BASELINE: Score = Score {
    branches: 1,
    returns: 0,
    adjustment: 0,
    total: 1,
};

/// An empty body scores the baseline in either result shape.
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

/// A generated event of any charge category leaves the baseline untouched.
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

/// An authored branch or guard raises the total by one.
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

/// A generated branch followed by an authored neutral keeps the baseline.
#[test]
fn generated_branch_then_authored_neutral_keeps_baseline() {
    let events = [generated(Observed::Branch), authored(Observed::Neutral)];

    assert_eq!(
        score(&events, false),
        BASELINE,
        "a neutral event resets only adjacency and never charges"
    );
}

/// A single authored return outside a Result body keeps the baseline total.
#[test]
fn authored_return_outside_result_keeps_baseline_total() {
    let events = [authored(Observed::Return)];

    assert_eq!(
        score(&events, false),
        Score {
            branches: 1,
            returns: 1,
            adjustment: 0,
            total: 1
        },
        "half of one return truncates away outside a Result body"
    );
}

/// A single authored return inside a Result body cancels the baseline.
#[test]
fn authored_return_inside_result_cancels_baseline() {
    let events = [authored(Observed::Return)];

    assert_eq!(
        score(&events, true),
        Score {
            branches: 1,
            returns: 1,
            adjustment: 1,
            total: 0
        },
        "a Result body credits a lone return in full"
    );
}

/// Consecutive authored returns charge once in either result shape.
#[rstest]
#[case::non_result(false, Score { branches: 1, returns: 1, adjustment: 0, total: 1 })]
#[case::result(true, Score { branches: 1, returns: 1, adjustment: 1, total: 0 })]
fn consecutive_authored_returns_charge_once(#[case] returns_result: bool, #[case] expected: Score) {
    let events = [authored(Observed::Return), authored(Observed::Return)];

    assert_eq!(
        score(&events, returns_result),
        expected,
        "only the first of two adjacent returns charges"
    );
}

/// An authored neutral between returns breaks their adjacency.
#[rstest]
#[case::non_result(false, Score { branches: 1, returns: 2, adjustment: 1, total: 0 })]
#[case::result(true, Score { branches: 1, returns: 2, adjustment: 2, total: 1 })]
fn authored_neutral_separates_returns(#[case] returns_result: bool, #[case] expected: Score) {
    let events = [
        authored(Observed::Return),
        authored(Observed::Neutral),
        authored(Observed::Return),
    ];

    assert_eq!(
        score(&events, returns_result),
        expected,
        "an authored neutral resets return adjacency"
    );
}

/// Generated events between returns do not break adjacency.
#[rstest]
#[case::non_result(false, Score { branches: 1, returns: 1, adjustment: 0, total: 1 })]
#[case::result(true, Score { branches: 1, returns: 1, adjustment: 1, total: 0 })]
fn generated_events_do_not_break_return_adjacency(
    #[case] returns_result: bool,
    #[case] expected: Score,
) {
    let events = [
        authored(Observed::Return),
        generated(Observed::Neutral),
        authored(Observed::Return),
    ];

    assert_eq!(
        score(&events, returns_result),
        expected,
        "an intervening generated event leaves returns adjacent"
    );
}

/// Three separated authored returns can outgrow the branch count.
#[test]
fn adjustment_larger_than_branches_keeps_branch_count() {
    let events = [
        authored(Observed::Return),
        authored(Observed::Neutral),
        authored(Observed::Return),
        authored(Observed::Neutral),
        authored(Observed::Return),
    ];

    assert_eq!(
        score(&events, true),
        Score {
            branches: 1,
            returns: 3,
            adjustment: 3,
            total: 1
        },
        "a total never falls below the branch count when the adjustment is larger"
    );
}

/// An authored branch and return in a Result body score one.
#[test]
fn authored_branch_and_return_in_result_body_score_one() {
    let events = [authored(Observed::Branch), authored(Observed::Return)];

    assert_eq!(
        score(&events, true),
        Score {
            branches: 2,
            returns: 1,
            adjustment: 1,
            total: 1
        },
        "one branch charge survives the return adjustment"
    );
}

/// Generated guards neither charge nor break return adjacency.
#[test]
fn generated_guards_do_not_charge_or_separate_returns() {
    let events = [
        authored(Observed::Return),
        generated(Observed::Guard),
        authored(Observed::Return),
    ];

    assert_eq!(
        score(&events, false),
        Score {
            branches: 1,
            returns: 1,
            adjustment: 0,
            total: 1
        },
        "a generated guard is not authored code"
    );
}

/// A total must strictly exceed a limit to report.
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
