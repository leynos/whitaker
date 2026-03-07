//! Shared test helpers for unit and integration tests.
//!
//! This module is `#[doc(hidden)]` and not part of the public API contract.
//! It exists solely to avoid duplicating test helper logic between the
//! `merge::tests` unit tests and the `tests/` integration tests.

use crate::builders::{LocationBuilder, RegionBuilder, ResultBuilder};
use crate::merge::WHITAKER_FRAGMENT_KEY;
use crate::model::result::{Level, SarifResult};

/// Builds a [`SarifResult`] with a fingerprint, location, and region.
///
/// Panics on builder failure; intended only for test code.
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
