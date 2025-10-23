//! Localisation loader and helpers for Whitaker diagnostics.
//!
//! The loader embeds Fluent resources under `locales/` so lint crates can
//! resolve translated strings without touching the filesystem at runtime. The
//! API exposes a thin wrapper around `fluent-templates` that tracks whether the
//! fallback bundle was used and surfaces missing message errors eagerly.

use fluent_templates::{
    loader::{LanguageIdentifier, Loader},
    static_loader,
};
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use unic_langid::langid;

/// Root directory containing the Fluent resources for all supported locales.
const LOCALES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../locales");

const FALLBACK_LITERAL: &str = "en-GB";

static_loader! {
    static LOADER = {
        locales: LOCALES_DIR,
        fallback_language: FALLBACK_LITERAL,
        // Retain Fluent's default Unicode isolating marks for bidi safety.
    };
}

/// The fallback locale bundled with every Whitaker build.
pub const FALLBACK_LOCALE: &str = FALLBACK_LITERAL;

const FALLBACK_LANGUAGE: LanguageIdentifier = langid!("en-GB");

static SUPPORTED: Lazy<Vec<LanguageIdentifier>> = Lazy::new(|| LOADER.locales().cloned().collect());

static SUPPORTED_STRINGS: Lazy<Vec<String>> = Lazy::new(|| {
    let mut locales: Vec<String> = SUPPORTED.iter().map(ToString::to_string).collect();
    locales.sort_unstable();
    locales
});

/// Error raised when localisation data cannot satisfy a caller request.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum I18nError {
    /// Raised when the requested message slug is missing for the resolved locale.
    #[error("message `{key}` missing for locale `{locale}`")]
    MissingMessage { key: String, locale: String },
}

/// HashMap wrapper used when passing Fluent arguments to lookups.
pub type Arguments<'a> = HashMap<Cow<'a, str>, fluent_bundle::FluentValue<'a>>;

/// Resolve localisation messages for a specific locale.
///
/// The loader eagerly falls back to `en-GB` when the requested locale is not
/// recognised. This mirrors the planned lookup order that surfaces explicit
/// configuration, environment overrides, and finally the bundled fallback.
#[derive(Clone, Debug)]
pub struct Localiser {
    language: LanguageIdentifier,
    fallback_used: bool,
}

impl Localiser {
    /// Create a localiser for `locale`, falling back to [`FALLBACK_LOCALE`].
    ///
    /// ```
    /// use common::i18n::{available_locales, Localiser};
    ///
    /// let locale = Localiser::new(Some("cy"));
    /// assert!(available_locales().contains(&"cy".to_string()));
    /// assert_eq!(locale.locale(), "cy");
    /// assert!(!locale.used_fallback());
    ///
    /// let fallback = Localiser::new(Some("zz"));
    /// assert_eq!(fallback.locale(), "en-GB");
    /// assert!(fallback.used_fallback());
    /// ```
    #[must_use]
    pub fn new(locale: Option<&str>) -> Self {
        let (language, fallback_used) = match locale
            .and_then(|value| LanguageIdentifier::from_str(value).ok())
            .filter(|identifier| is_supported(identifier))
        {
            Some(identifier) => (identifier, false),
            None => (FALLBACK_LANGUAGE.clone(), true),
        };

        Self {
            language,
            fallback_used,
        }
    }

    /// Return the resolved locale identifier.
    #[must_use]
    pub fn language(&self) -> &LanguageIdentifier {
        &self.language
    }

    /// Return the resolved locale as a string slice.
    #[must_use]
    pub fn locale(&self) -> &str {
        self.language.as_ref()
    }

    /// Whether the fallback locale was used.
    #[must_use]
    pub fn used_fallback(&self) -> bool {
        self.fallback_used
    }

    /// Fetch the translated message for `key`.
    pub fn message(&self, key: &str) -> Result<String, I18nError> {
        self.lookup(key, None)
    }

    /// Fetch the translated message with Fluent arguments.
    pub fn message_with_args(&self, key: &str, args: &Arguments<'_>) -> Result<String, I18nError> {
        self.lookup(key, Some(args))
    }

    /// Fetch a translated attribute, e.g. `function.primary`.
    pub fn attribute(&self, key: &str, attribute: &str) -> Result<String, I18nError> {
        self.message(&compose_attribute_key(key, attribute))
    }

    /// Fetch a translated attribute with Fluent arguments.
    pub fn attribute_with_args(
        &self,
        key: &str,
        attribute: &str,
        args: &Arguments<'_>,
    ) -> Result<String, I18nError> {
        self.message_with_args(&compose_attribute_key(key, attribute), args)
    }

    fn lookup(&self, key: &str, args: Option<&Arguments<'_>>) -> Result<String, I18nError> {
        let maybe_value = match args {
            Some(arguments) => LOADER.try_lookup_with_args(&self.language, key, arguments),
            None => LOADER.try_lookup(&self.language, key),
        };

        maybe_value.ok_or_else(|| I18nError::MissingMessage {
            key: key.to_owned(),
            locale: self.language.to_string(),
        })
    }
}

fn compose_attribute_key(key: &str, attribute: &str) -> String {
    let mut composed = String::with_capacity(key.len() + attribute.len() + 1);
    fmt::write(&mut composed, format_args!("{key}.{attribute}"))
        .expect("writing to string cannot fail");
    composed
}

fn is_supported(locale: &LanguageIdentifier) -> bool {
    SUPPORTED.iter().any(|candidate| candidate == locale)
}

/// Return a sorted slice of the available locales.
#[must_use]
pub fn available_locales() -> &'static [String] {
    SUPPORTED_STRINGS.as_slice()
}

/// Check whether a locale tag is supported by the embedded bundles.
#[must_use]
pub fn supports_locale(locale: &str) -> bool {
    LanguageIdentifier::from_str(locale)
        .ok()
        .map_or(false, |identifier| is_supported(&identifier))
}

#[cfg(test)]
mod tests {
    use super::{FALLBACK_LOCALE, Localiser, available_locales, supports_locale};
    use fluent_bundle::FluentValue;
    use rstest::rstest;
    use std::borrow::Cow;

    #[rstest]
    #[case(None, FALLBACK_LOCALE, true)]
    #[case(Some("en-GB"), "en-GB", false)]
    #[case(Some("cy"), "cy", false)]
    #[case(Some("gd"), "gd", false)]
    #[case(Some("zz"), FALLBACK_LOCALE, true)]
    fn resolves_locales(
        #[case] input: Option<&str>,
        #[case] expected: &str,
        #[case] fallback: bool,
    ) {
        let localiser = Localiser::new(input);
        assert_eq!(localiser.locale(), expected);
        assert_eq!(localiser.used_fallback(), fallback);
    }

    #[test]
    fn enumerates_available_locales() {
        let locales = available_locales();
        assert!(locales.contains(&"en-GB".to_string()));
        assert!(locales.contains(&"cy".to_string()));
        assert!(locales.contains(&"gd".to_string()));
    }

    #[test]
    fn supports_locale_reports_known_languages() {
        assert!(supports_locale("en-GB"));
        assert!(supports_locale("cy"));
        assert!(supports_locale("gd"));
        assert!(!supports_locale("zz"));
    }

    #[test]
    fn message_lookup_with_arguments_interpolates_values() {
        let localiser = Localiser::new(Some("gd"));
        let mut args = super::Arguments::new();
        args.insert(
            Cow::Borrowed("lint"),
            FluentValue::from("function_attrs_follow_docs"),
        );
        let message = localiser
            .message_with_args("common-lint-count", &args)
            .expect("message should exist");
        assert!(message.contains("function_attrs_follow_docs"));
    }
}
