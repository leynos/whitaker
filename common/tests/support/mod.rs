//! Test support utilities for i18n and locale parsing.
//! Re-exports FTL parsing helpers and locale discovery utilities consumed by
//! localisation behaviour and quality tests.

pub mod i18n_ftl;
pub mod i18n_helpers;

#[expect(
    unused_imports,
    reason = "Test utilities re-exported for selective use across test modules"
)]
pub use i18n_ftl::*;
pub use i18n_helpers::*;
