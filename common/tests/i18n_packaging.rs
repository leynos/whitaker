//! Regression coverage for crate-relative localisation packaging.
//!
//! The `whitaker-common` crate is published independently, so its Fluent
//! bundles must live under the crate root rather than elsewhere in the
//! workspace. This keeps `cargo package` verification aligned with local test
//! runs.

use std::path::PathBuf;

#[test]
fn fluent_bundles_live_under_the_common_crate_root() {
    let locales_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales");

    assert!(
        locales_root.is_dir(),
        "expected crate-local locales directory at {}",
        locales_root.display()
    );
    assert!(
        locales_root.join("en-GB/common.ftl").is_file(),
        "expected fallback common bundle under {}",
        locales_root.display()
    );
}
