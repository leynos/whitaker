//! Shared test helpers for unit and integration tests.
//!
//! This module is `#[doc(hidden)]` and not part of the public API contract.
//! It exists solely to avoid duplicating test helper logic between the
//! `merge::tests` unit tests and the `tests/` integration tests.

use core::fmt::Debug;

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    builders::{LocationBuilder, RegionBuilder, ResultBuilder},
    merge::WHITAKER_FRAGMENT_KEY,
    model::result::{Level, SarifResult},
};

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
/// Intended only for test code.
///
/// # Panics
///
/// Panics if the region or result builder rejects its inputs, which indicates
/// a defect in the test fixture rather than a recoverable condition.
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
