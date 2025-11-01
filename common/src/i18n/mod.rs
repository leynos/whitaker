//! Localisation loader and helpers for Whitaker diagnostics.
//!
//! The loader embeds Fluent resources under `locales/` so lint crates can
//! resolve translated strings without touching the filesystem at runtime. The
//! API exposes a thin wrapper around `fluent-templates` that tracks whether the
//! fallback bundle was used and surfaces missing message errors eagerly.
//!
//! Locale resolution is handled by [`resolve_localiser`], which evaluates
//! explicit overrides, environment variables, and configuration settings in
//! priority order before falling back to the bundled locale.
//!
//! See [`resolve_message_set`] for fetching a lint’s primary/note/help trio.

use fluent_templates::static_loader;
use unic_langid::langid;

/// Re-export the Fluent value type for constructing diagnostic arguments.
/// See [`resolve_message_set`] for loading messages that consume these
/// arguments.
pub use fluent_templates::fluent_bundle::FluentValue;
pub(crate) use fluent_templates::loader::LanguageIdentifier;

const FALLBACK_LITERAL: &str = "en-GB";

static_loader! {
    pub(crate) static LOADER = {
        locales: "../locales",
        fallback_language: "en-GB",
        // Retain Fluent's default Unicode isolating marks for bidi safety.
    };
}

pub const FALLBACK_LOCALE: &str = FALLBACK_LITERAL;
pub(crate) const FALLBACK_LANGUAGE: LanguageIdentifier = langid!("en-GB");

mod diagnostics;
mod loader;
mod locales;
mod selection;
pub mod testing;

/// Diagnostic localisation helpers.
/// See [`resolve_message_set`] for fetching primary, note, and help strings.
pub use diagnostics::{
    AttrKey, BundleLookup, DiagnosticMessageSet, MessageKey, resolve_message_set,
};
pub use loader::{Arguments, I18nError, Localiser};
pub use locales::{available_locales, supports_locale};
pub use selection::{LocaleSelection, LocaleSource, normalise_locale, resolve_localiser};

#[cfg(test)]
mod tests;
