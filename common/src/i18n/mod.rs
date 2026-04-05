//! Localisation loader and helpers for Whitaker diagnostics.
//!
//! The loader embeds Fluent resources under the crate-local `locales/`
//! directory so lint crates can resolve translated strings without touching
//! the filesystem at runtime. The API exposes a thin wrapper around
//! `fluent-templates` that tracks whether the fallback bundle was used and
//! surfaces missing message errors eagerly.
//!
//! Locale resolution is handled by [`resolve_localizer`], which evaluates
//! explicit overrides, environment variables, and configuration settings in
//! priority order before falling back to the bundled locale.
//!
//! See [`resolve_message_set`] for fetching a lint’s primary/note/help trio.

use fluent_templates::static_loader;
use std::path::PathBuf;
use unic_langid::langid;

/// Re-export the Fluent value type for constructing diagnostic arguments.
/// See [`resolve_message_set`] for loading messages that consume these
/// arguments.
pub use fluent_templates::fluent_bundle::FluentValue;
pub(crate) use fluent_templates::loader::LanguageIdentifier;

const FALLBACK_LITERAL: &str = "en-GB";
/// Directory name used for Fluent locale resources.
pub const LOCALES_DIR_NAME: &str = "locales";
/// Default Fluent bundle filename for diagnostics resources.
pub const LOCALES_FTL_FILE: &str = "common.ftl";

static_loader! {
    pub(crate) static LOADER = {
        locales: "locales",
        fallback_language: "en-GB",
    };
}

pub const FALLBACK_LOCALE: &str = FALLBACK_LITERAL;
pub(crate) const FALLBACK_LANGUAGE: LanguageIdentifier = langid!("en-GB");

/// Return the crate-local Fluent resource root used by packaging and tests.
pub fn locales_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(LOCALES_DIR_NAME)
}

/// Return the relative path inside the package tarball for a locale bundle.
pub fn packaged_locale_path(locale: &str, file: &str) -> PathBuf {
    PathBuf::from(format!(
        "{}-{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    ))
    .join(LOCALES_DIR_NAME)
    .join(locale)
    .join(file)
}

/// Return the default packaged locale path for Whitaker diagnostics.
pub fn packaged_fallback_locale_path() -> PathBuf {
    packaged_locale_path(FALLBACK_LOCALE, LOCALES_FTL_FILE)
}

mod diagnostics;
mod helpers;
mod loader;
mod locales;
mod selection;
pub mod testing;

/// Diagnostic localisation helpers.
/// See [`resolve_message_set`] for fetching primary, note, and help strings.
pub use diagnostics::{
    AttrKey, BundleLookup, DiagnosticMessageSet, MessageKey, resolve_message_set,
};
pub use helpers::{
    MessageResolution, branch_phrase, get_localizer_for_lint, noop_reporter,
    safe_resolve_message_set,
};
pub use loader::{Arguments, I18nError, Localizer};
pub use locales::{available_locales, supports_locale};
pub use selection::{LocaleSelection, LocaleSource, normalise_locale, resolve_localizer};

#[cfg(test)]
mod tests;
