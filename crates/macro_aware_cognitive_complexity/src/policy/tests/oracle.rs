//! An independent scoring reference for property tests.

use whitaker_common::node_origin::NodeOrigin;

use crate::policy::{Event, Observed, Score};

/// Compute the expected score by filtering authored events first and then
/// counting them with `enumerate`-based adjacency.
///
/// This deliberately avoids the production state machine: it materialises the
/// authored projection, folds branch charges, and decides each return against
/// its predecessor by index, so an error in the production loop cannot hide.
pub(super) fn expected_score(events: &[Event], returns_result: bool) -> Score {
    let authored: Vec<Event> = events
        .iter()
        .copied()
        .filter(|event| event.origin == NodeOrigin::Authored)
        .collect();

    let branches = authored
        .iter()
        .filter(|event| matches!(event.observed, Observed::Branch | Observed::Guard))
        .fold(1_u64, |total, _event| total.saturating_add(1));

    let returns = authored
        .iter()
        .enumerate()
        .filter(|(index, event)| {
            event.observed == Observed::Return && is_discontinuous(&authored, *index)
        })
        .fold(0_u64, |total, _event| total.saturating_add(1));

    let adjustment = if returns_result {
        returns
    } else {
        // Euclidean division equals truncation for unsigned values.
        returns.div_euclid(2)
    };
    let total = if branches >= adjustment {
        branches - adjustment
    } else {
        branches
    };

    Score {
        branches,
        returns,
        adjustment,
        total,
    }
}

/// Report whether the event at `index` is not preceded by another return.
fn is_discontinuous(authored: &[Event], index: usize) -> bool {
    index
        .checked_sub(1)
        .and_then(|previous| authored.get(previous))
        .is_none_or(|previous| previous.observed != Observed::Return)
}
