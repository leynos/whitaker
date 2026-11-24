//! Typed model of the SARIF `baselineState` value set.
//!
//! SARIF 2.1.0 §3.27.24 defines a closed set of baseline states. Modelling the
//! set as an enum keeps a mistyped string from silently corrupting baseline
//! semantics for every consumer of the log.
//!
//! # Examples
//!
//! ```
//! use whitaker_sarif::BaselineState;
//!
//! let json = serde_json::to_string(&BaselineState::Updated).expect("serialize");
//! assert_eq!(json, "\"updated\"");
//! ```

use serde::{Deserialize, Serialize};

/// Baseline comparison state of a SARIF result.
///
/// Run 1 of the clone detector emits [`BaselineState::New`] for pairs the AST
/// pass added, [`BaselineState::Unchanged`] for results carried from Run 0 and
/// [`BaselineState::Updated`] for results it reclassified. The
/// [`BaselineState::Absent`] variant completes the SARIF value set; Whitaker
/// never emits it, because Run 1 never removes a token-pass result.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::BaselineState;
///
/// assert_eq!(BaselineState::New, BaselineState::New);
/// assert_ne!(BaselineState::New, BaselineState::Updated);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BaselineState {
    /// The result is new relative to the baseline.
    New,
    /// The result is unchanged since the baseline.
    Unchanged,
    /// The result existed in the baseline and has changed.
    Updated,
    /// The result existed in the baseline and is now absent.
    Absent,
}

#[cfg(test)]
mod tests {
    //! Round-trip and vocabulary tests for the typed baseline state.

    use rstest::rstest;

    use super::*;
    use crate::test_support::{assert_json_round_trip, assert_serialized_json};

    /// Check that every state serializes with its SARIF spelling.
    #[rstest]
    #[case(BaselineState::New, "\"new\"")]
    #[case(BaselineState::Unchanged, "\"unchanged\"")]
    #[case(BaselineState::Updated, "\"updated\"")]
    #[case(BaselineState::Absent, "\"absent\"")]
    fn states_serialize_in_lower_camel_case(#[case] state: BaselineState, #[case] expected: &str) {
        assert_serialized_json(&state, |json| {
            assert_eq!(json, expected, "unexpected serialization of {state:?}");
        });
    }

    /// Check that every state survives a JSON round trip.
    #[rstest]
    #[case(BaselineState::New)]
    #[case(BaselineState::Unchanged)]
    #[case(BaselineState::Updated)]
    #[case(BaselineState::Absent)]
    fn states_round_trip(#[case] state: BaselineState) { assert_json_round_trip(&state); }

    /// Check that an unknown baseline string is rejected rather than coerced.
    #[test]
    fn unknown_state_is_rejected() {
        let decoded = serde_json::from_str::<BaselineState>("\"superseded\"");
        assert!(
            decoded.is_err(),
            "an unknown baseline state must not deserialize, got {decoded:?}"
        );
    }
}
