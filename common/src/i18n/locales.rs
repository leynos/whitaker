//! Locale enumeration and validation.
//!
//! This module exposes the set of available locales embedded in the Fluent
//! bundles and provides utilities for checking whether a given locale tag is
//! supported.

use once_cell::sync::Lazy;

use fluent_templates::{Loader, loader::LanguageIdentifier};

use super::LOADER;

static ALL_LOCALES: Lazy<Vec<String>> = Lazy::new(|| {
    let mut locales: Vec<String> = LOADER.locales().map(|id| id.to_string()).collect();
    locales.sort_unstable();
    locales
});

/// Return a sorted slice of the available locales.
#[must_use]
pub fn available_locales() -> &'static [String] {
    ALL_LOCALES.as_slice()
}

/// Check whether a locale tag is supported by the embedded bundles.
#[must_use]
pub fn supports_locale(locale: &str) -> bool {
    match locale.parse::<LanguageIdentifier>() {
        Ok(identifier) => {
            let canonical = identifier.to_string();
            ALL_LOCALES
                .binary_search_by(|candidate| candidate.as_str().cmp(canonical.as_str()))
                .is_ok()
        }
        Err(_) => false,
    }
}
