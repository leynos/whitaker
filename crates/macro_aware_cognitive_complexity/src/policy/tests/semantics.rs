//! Semantic predicates stated with googletest matchers.

use googletest::prelude::*;

use super::support::{authored, generated};
use crate::policy::{Observed, Score, score};

/// Verify that generated events alone cannot move the score away from the baseline.
#[gtest]
fn generated_only_events_preserve_the_baseline() {
    let baseline = Score {
        branches: 1,
        returns: 0,
        adjustment: 0,
        total: 1,
    };
    let events = [
        generated(Observed::Branch),
        generated(Observed::Guard),
        generated(Observed::Return),
        generated(Observed::Neutral),
    ];

    expect_that!(score(&events, false), eq(baseline));
    expect_that!(score(&events, true), eq(baseline));
}

/// Verify that an authored branch raises the total unless the adjustment absorbs it.
#[gtest]
fn authored_branch_raises_total_unless_the_adjustment_absorbs_it() {
    let branch = [authored(Observed::Branch)];
    expect_that!(score(&branch, false).total, eq(2));

    let branch_and_return = [authored(Observed::Branch), authored(Observed::Return)];
    expect_that!(score(&branch_and_return, true).total, eq(1));
}

/// Verify that generated events between returns never divide their adjacency.
#[gtest]
fn generated_events_never_break_return_adjacency() {
    let adjacent_returns = [authored(Observed::Return), authored(Observed::Return)];
    let separated_by_generated = [
        authored(Observed::Return),
        generated(Observed::Neutral),
        generated(Observed::Guard),
        authored(Observed::Return),
    ];

    expect_that!(score(&separated_by_generated, false).returns, eq(1));
    expect_that!(
        score(&separated_by_generated, true),
        eq(score(&adjacent_returns, true)),
    );
}
