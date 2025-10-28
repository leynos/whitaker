//! Localisation loader and helpers for Whitaker diagnostics.
//!
//! The loader embeds Fluent resources under `locales/` so lint crates can
//! resolve translated strings without touching the filesystem at runtime. The
//! API exposes a thin wrapper around `fluent-templates` that tracks whether the
//! fallback bundle was used and surfaces missing message errors eagerly.
//!
//! See [`diagnostics::resolve_message_set`] for fetching a lint's primary,
//! note, and help diagnostics in one call.

use fluent_templates::static_loader;
use unic_langid::langid;

/// Re-export the Fluent value type for constructing diagnostic arguments.
/// See [`diagnostics::resolve_message_set`] for loading messages that consume
/// these arguments.
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

pub use diagnostics::{
    AttrKey, BundleLookup, DiagnosticMessageSet, MessageKey, resolve_message_set,
};
pub use loader::{Arguments, I18nError, Localiser};
pub use locales::{available_locales, supports_locale};

#[cfg(test)]
mod tests;
