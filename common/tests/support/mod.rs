//! Test support utilities for i18n and locale parsing.
//! Re-exports FTL parsing helpers and locale discovery utilities consumed by
//! localisation behaviour and quality tests.
#![allow(unused_imports)]

pub mod i18n_ftl;
pub mod i18n_helpers;

pub use i18n_ftl::*;
pub use i18n_helpers::*;
