use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;

use fluent_bundle::FluentValue;
use thiserror::Error;

use super::locales::supports_locale;
use super::{FALLBACK_LANGUAGE, LOADER, LanguageIdentifier};

/// HashMap wrapper used when passing Fluent arguments to lookups.
pub type Arguments<'a> = HashMap<Cow<'a, str>, FluentValue<'a>>;

/// Error raised when localisation data cannot satisfy a caller request.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum I18nError {
    /// Raised when the requested message slug is missing for the resolved locale.
    #[error("message `{key}` missing for locale `{locale}`")]
    MissingMessage { key: String, locale: String },
}

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
        match locale {
            Some(value) if supports_locale(value) => match LanguageIdentifier::from_str(value) {
                Ok(identifier) => Self {
                    language: identifier,
                    fallback_used: false,
                },
                Err(_) => Self::fallback(),
            },
            _ => Self::fallback(),
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
        self.lookup(key, None, None)
    }

    /// Fetch the translated message with Fluent arguments.
    pub fn message_with_args(&self, key: &str, args: &Arguments<'_>) -> Result<String, I18nError> {
        self.lookup(key, None, Some(args))
    }

    /// Fetch a translated attribute, e.g. `function.primary`.
    pub fn attribute(&self, key: &str, attribute: &str) -> Result<String, I18nError> {
        self.lookup(key, Some(attribute), None)
    }

    /// Fetch a translated attribute with Fluent arguments.
    pub fn attribute_with_args(
        &self,
        key: &str,
        attribute: &str,
        args: &Arguments<'_>,
    ) -> Result<String, I18nError> {
        self.lookup(key, Some(attribute), Some(args))
    }

    fn lookup(
        &self,
        key: &str,
        attribute: Option<&str>,
        args: Option<&Arguments<'_>>,
    ) -> Result<String, I18nError> {
        let lookup_key = attribute
            .map(|attr| format!("{key}.{attr}"))
            .unwrap_or_else(|| key.to_string());

        let maybe_value = match args {
            Some(arguments) => {
                LOADER.try_lookup_with_args(&self.language, lookup_key.as_str(), arguments)
            }
            None => LOADER.try_lookup(&self.language, lookup_key.as_str()),
        };

        maybe_value.ok_or_else(|| I18nError::MissingMessage {
            key: lookup_key,
            locale: self.language.to_string(),
        })
    }

    fn fallback() -> Self {
        Self {
            language: FALLBACK_LANGUAGE.clone(),
            fallback_used: true,
        }
    }
}
