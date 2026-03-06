//! Shared test helpers for `whitaker_sarif` integration tests.

use whitaker_sarif::{Level, LocationBuilder, RegionBuilder, ResultBuilder, SarifResult};

/// Builds a [`SarifResult`] with a fingerprint, location, and region.
///
/// Panics on builder failure.
pub fn make_keyed_result(rule: &str, file: &str, line: usize, fp: &str) -> SarifResult {
    let region = match RegionBuilder::new(line).with_end_line(line + 5).build() {
        Ok(r) => r,
        Err(e) => panic!("failed to build region: {e}"),
    };
    match ResultBuilder::new(rule)
        .with_message("clone detected")
        .with_level(Level::Warning)
        .with_location(LocationBuilder::new(file).with_region(region).build())
        .with_fingerprint("whitakerFragment", fp)
        .build()
    {
        Ok(result) => result,
        Err(e) => panic!("failed to build keyed result: {e}"),
    }
}
