//! Shared test helpers for unit and integration tests.
//!
//! This module is `#[doc(hidden)]` and not part of the public API contract.
//! It exists solely to avoid duplicating test helper logic between the
//! `merge::tests` unit tests and the `tests/` integration tests.

use core::fmt::Debug;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::builders::{LocationBuilder, RegionBuilder, ResultBuilder};
use crate::merge::WHITAKER_FRAGMENT_KEY;
use crate::model::result::{Level, SarifResult};

/// Asserts that `value` survives a JSON serialize/deserialize round trip
/// unchanged.
///
/// Panics with the type name and underlying serde error on failure; intended
/// only for test code.
pub fn assert_json_round_trip<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let type_name = core::any::type_name::<T>();
    let json = match serde_json::to_string(value) {
        Ok(json) => json,
        Err(e) => panic!("failed to serialize {type_name}: {e}"),
    };
    match serde_json::from_str::<T>(&json) {
        Ok(parsed) => assert_eq!(
            *value, parsed,
            "JSON round trip changed {type_name} (json: {json})"
        ),
        Err(e) => panic!("failed to deserialize {type_name} from {json}: {e}"),
    }
}

/// Serializes `value` to JSON and passes the resulting string to `check`.
///
/// Panics with the underlying serde error if serialization fails; intended
/// only for test code.
pub fn assert_serialized_json(value: &impl Serialize, check: impl FnOnce(&str)) {
    match serde_json::to_string(value) {
        Ok(json) => check(&json),
        Err(e) => panic!("failed to serialize value to JSON: {e}"),
    }
}

/// Builds a [`SarifResult`] with a fingerprint, location, and region.
///
/// Computes the end line as `line + 5`; integer overflow follows the active
/// Rust overflow-check configuration. Intended only for test code.
///
/// # Panics
///
/// Panics if a region or result builder rejects its input, which indicates a
/// defect in the test fixture rather than a recoverable condition.
#[must_use]
pub fn make_keyed_result(rule: &str, file: &str, line: usize, fp: &str) -> SarifResult {
    let region = match RegionBuilder::new(line).with_end_line(line + 5).build() {
        Ok(r) => r,
        Err(e) => panic!("failed to build region: {e}"),
    };
    match ResultBuilder::new(rule)
        .with_message("clone detected")
        .with_level(Level::Warning)
        .with_location(LocationBuilder::new(file).with_region(region).build())
        .with_fingerprint(WHITAKER_FRAGMENT_KEY, fp)
        .build()
    {
        Ok(result) => result,
        Err(e) => panic!("failed to build keyed result: {e}"),
    }
}

#[cfg(test)]
mod tests {
    //! Verify the shared JSON assertions against successful and failing cases.

    use core::cell::Cell;

    use serde::{Deserialize, Serialize};

    use super::{assert_json_round_trip, assert_serialized_json};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        count: u8,
    }

    #[derive(Debug, PartialEq, Serialize)]
    struct ChangesOnRead(u8);

    impl<'de> Deserialize<'de> for ChangesOnRead {
        /// Parse the value and substitute zero to exercise the equality check.
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let _serialized_value = u8::deserialize(deserializer)?;
            Ok(Self(0))
        }
    }

    /// Check that a JSON round trip preserves a representative value.
    #[test]
    fn json_round_trip_accepts_equal_values() {
        let sample = Sample { count: 3 };
        assert_json_round_trip(&sample);
    }

    /// Check that a changed value makes the JSON round-trip assertion fail.
    #[test]
    #[should_panic(expected = "JSON round trip changed")]
    fn json_round_trip_rejects_changed_values() {
        assert_json_round_trip(&ChangesOnRead(3));
    }

    /// Check the serialized payload and verify that the callback runs.
    #[test]
    fn serialized_json_is_passed_to_the_callback() {
        let callback_was_called = Cell::new(false);
        assert_serialized_json(&Sample { count: 3 }, |json| {
            assert_eq!(json, r#"{"count":3}"#);
            callback_was_called.set(true);
        });

        assert!(callback_was_called.get());
    }
}
