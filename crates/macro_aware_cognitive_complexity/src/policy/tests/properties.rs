//! Property tests for the scoring contract.

use proptest::prelude::*;
use whitaker_common::node_origin::NodeOrigin;

use super::oracle::expected_score;
use crate::policy::{ChargeLedger, Event, Observed, score};

/// Generate either provenance.
fn origin_strategy() -> impl Strategy<Value = NodeOrigin> {
    prop_oneof![Just(NodeOrigin::Authored), Just(NodeOrigin::Generated)]
}

/// Generate any charge category.
fn observed_strategy() -> impl Strategy<Value = Observed> {
    prop_oneof![
        Just(Observed::Branch),
        Just(Observed::Guard),
        Just(Observed::Return),
        any::<bool>().prop_map(|returns_result| Observed::BlockReturn { returns_result }),
        Just(Observed::Neutral),
    ]
}

/// Generate one event with an independent category and provenance.
fn event_strategy() -> impl Strategy<Value = Event> {
    (observed_strategy(), origin_strategy())
        .prop_map(|(observed, origin)| Event::new(observed, origin))
}

/// Generate an authored event that changes the branch or return count.
fn authored_charge_strategy() -> impl Strategy<Value = Event> {
    prop_oneof![Just(Observed::Branch), Just(Observed::Return)]
        .prop_map(|observed| Event::new(observed, NodeOrigin::Authored))
}

proptest! {
    /// Verify that arbitrary streams agree with the independent oracle.
    #[test]
    fn score_matches_the_oracle(
        events in prop::collection::vec(event_strategy(), 0..=256_usize),
        returns_result in any::<bool>(),
    ) {
        prop_assert_eq!(
            score(&events, returns_result),
            expected_score(&events, returns_result),
        );
    }

    /// Verify that a generated event inserted anywhere leaves the score unchanged.
    #[test]
    fn inserting_a_generated_event_preserves_the_score(
        events in prop::collection::vec(event_strategy(), 0..=256_usize),
        position in 0..=256_usize,
        observed in observed_strategy(),
        returns_result in any::<bool>(),
    ) {
        let expected = score(&events, returns_result);
        let insert_at = position.min(events.len());
        let mut extended = events;
        extended.insert(insert_at, Event::new(observed, NodeOrigin::Generated));

        prop_assert_eq!(score(&extended, returns_result), expected);
    }

    /// Verify that an authored neutral between two returns separates their adjacency.
    #[test]
    fn authored_neutral_between_returns_separates_adjacency(
        prefix in prop::collection::vec(event_strategy(), 0..=32_usize),
        suffix in prop::collection::vec(event_strategy(), 0..=32_usize),
        returns_result in any::<bool>(),
    ) {
        let first_return = prefix.len();
        let return_event = Event::new(Observed::Return, NodeOrigin::Authored);
        let mut joined = prefix;
        joined.push(return_event);
        joined.push(return_event);
        joined.extend(suffix);
        let before = score(&joined, returns_result);

        let insert_at = first_return + 1;
        joined.insert(insert_at, Event::new(Observed::Neutral, NodeOrigin::Authored));
        let after = score(&joined, returns_result);

        prop_assert_eq!(after.returns, before.returns.saturating_add(1));
        prop_assert_eq!(after, expected_score(&joined, returns_result));
    }

    /// Verify that the structural invariants hold for every stream.
    #[test]
    fn structural_invariants_hold(
        events in prop::collection::vec(event_strategy(), 0..=256_usize),
        returns_result in any::<bool>(),
    ) {
        let computed = score(&events, returns_result);

        prop_assert!(computed.total <= computed.branches, "total exceeds branches");
        prop_assert!(computed.branches >= 1, "the baseline branch charge is missing");
        prop_assert!(
            computed.adjustment <= computed.returns,
            "adjustment exceeds the return count",
        );
    }

    /// Verify that a long stream of authored charges stays representable and
    /// agrees with the oracle under saturating arithmetic.
    #[test]
    fn long_authored_stream_stays_within_u64(
        events in prop::collection::vec(authored_charge_strategy(), 0..=256_usize),
        returns_result in any::<bool>(),
    ) {
        let computed = score(&events, returns_result);

        prop_assert_eq!(computed, expected_score(&events, returns_result));
        prop_assert!(computed.branches >= 1, "the baseline branch charge is missing");
    }

    /// Verify that each authored charge position is kept once and no other is dropped.
    #[test]
    fn the_ledger_keeps_each_authored_charge_once(
        visits in prop::collection::vec(visit_strategy(), 0..=128_usize),
    ) {
        let kept = kept_events(&visits);
        let distinct: std::collections::HashSet<_> = visits
            .iter()
            .filter(|visit| charges(visit))
            .map(|visit| (std::mem::discriminant(&visit.observed), visit.lo, visit.hi))
            .collect();
        let kept_charges = visits
            .iter()
            .zip(&kept)
            .filter(|(visit, is_kept)| charges(visit) && **is_kept)
            .count();

        prop_assert_eq!(kept_charges, distinct.len());
    }

    /// Verify that neutral and generated visits are never reported as repeats.
    #[test]
    fn the_ledger_never_drops_neutral_or_generated_visits(
        visits in prop::collection::vec(visit_strategy(), 0..=128_usize),
    ) {
        let kept = kept_events(&visits);

        for (visit, is_kept) in visits.iter().zip(kept) {
            if !charges(visit) {
                prop_assert!(is_kept);
            }
        }
    }

    /// Verify that expanding the same visits twice scores the same as once.
    ///
    /// Dropping the repeated copy must leave return adjacency alone, so the
    /// score of a doubled walk equals the score of a single one.
    #[test]
    fn a_doubled_walk_scores_as_a_single_walk(
        visits in prop::collection::vec(visit_strategy(), 0..=64_usize),
        returns_result in any::<bool>(),
    ) {
        let single = surviving_events(&visits);
        let doubled_visits: Vec<Visit> = visits.iter().chain(&visits).copied().collect();
        let doubled = surviving_events(&doubled_visits);

        prop_assert_eq!(score(&doubled, returns_result), score(&single, returns_result));
    }
}

/// One walker visit: a charge category, provenance and source position.
#[derive(Clone, Copy, Debug)]
struct Visit {
    observed: Observed,
    origin: NodeOrigin,
    lo: u32,
    hi: u32,
}

/// Generate visits over a few positions, so repeats are common.
fn visit_strategy() -> impl Strategy<Value = Visit> {
    (observed_strategy(), origin_strategy(), 0_u32..4, 0_u32..4).prop_map(
        |(observed, origin, lo, hi)| Visit {
            observed,
            origin,
            lo,
            hi,
        },
    )
}

/// Report whether a visit is an authored charge the ledger tracks.
fn charges(visit: &Visit) -> bool {
    visit.origin == NodeOrigin::Authored && visit.observed != Observed::Neutral
}

/// Report, per visit, whether the ledger keeps it or reports a repeat.
fn kept_events(visits: &[Visit]) -> Vec<bool> {
    let mut ledger = ChargeLedger::default();
    visits
        .iter()
        .map(|visit| {
            !ledger.is_repeat(Event::new(visit.observed, visit.origin), visit.lo, visit.hi)
        })
        .collect()
}

/// Build the event stream the walker would record, dropping repeated visits.
fn surviving_events(visits: &[Visit]) -> Vec<Event> {
    visits
        .iter()
        .zip(kept_events(visits))
        .filter(|(_visit, is_kept)| *is_kept)
        .map(|(visit, _is_kept)| Event::new(visit.observed, visit.origin))
        .collect()
}
