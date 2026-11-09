//! Property tests for the scoring contract.

use proptest::prelude::*;
use whitaker_common::node_origin::NodeOrigin;

use super::oracle::expected_score;
use crate::policy::{Event, Observed, score};

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
    /// Arbitrary streams agree with the independent oracle.
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

    /// A generated event inserted anywhere leaves the score unchanged.
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

    /// An authored neutral between two returns separates their adjacency.
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

    /// Structural invariants hold for every stream.
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

    /// A long stream of authored charges stays representable and agrees with
    /// the oracle under saturating arithmetic.
    #[test]
    fn long_authored_stream_stays_within_u64(
        events in prop::collection::vec(authored_charge_strategy(), 0..=256_usize),
        returns_result in any::<bool>(),
    ) {
        let computed = score(&events, returns_result);

        prop_assert_eq!(computed, expected_score(&events, returns_result));
        prop_assert!(computed.branches >= 1, "the baseline branch charge is missing");
    }
}
