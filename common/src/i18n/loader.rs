use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;

use fluent_templates::{Loader, fluent_bundle::FluentValue};
use thiserror::Error;

use super::locales::supports_locale;
use super::{FALLBACK_LANGUAGE, FALLBACK_LITERAL, LOADER, LanguageIdentifier};

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
pub struct Localizer {
    language: LanguageIdentifier,
    language_tag: String,
    fallback_used: bool,
}

impl Localizer {
    /// Create a localizer for `locale`, falling back to [`crate::i18n::FALLBACK_LOCALE`].
    ///
    /// ```
    /// use common::i18n::{available_locales, Localizer};
    ///
    /// let locale = Localizer::new(Some("cy"));
    /// assert!(available_locales().contains(&"cy".to_string()));
    /// assert_eq!(locale.locale(), "cy");
    /// assert!(!locale.used_fallback());
    ///
    /// let fallback = Localizer::new(Some("zz"));
    /// assert_eq!(fallback.locale(), "en-GB");
    /// assert!(fallback.used_fallback());
    /// ```
    #[must_use]
    pub fn new(locale: Option<&str>) -> Self {
        match locale {
            Some(value) if supports_locale(value) => match LanguageIdentifier::from_str(value) {
                Ok(identifier) => {
                    let language_tag = identifier.to_string();

                    Self {
                        language: identifier,
                        language_tag,
                        fallback_used: false,
                    }
                }
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
        &self.language_tag
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
                let owned_arguments = promote_arguments(arguments);
                LOADER.try_lookup_with_args(&self.language, lookup_key.as_str(), &owned_arguments)
            }
            None => LOADER.try_lookup(&self.language, lookup_key.as_str()),
        };

        maybe_value.ok_or_else(|| I18nError::MissingMessage {
            key: lookup_key,
            locale: self.language_tag.clone(),
        })
    }

    fn fallback() -> Self {
        Self {
            language: FALLBACK_LANGUAGE.clone(),
            language_tag: FALLBACK_LITERAL.to_string(),
            fallback_used: true,
        }
    }
}

fn promote_arguments(
    arguments: &Arguments<'_>,
) -> HashMap<Cow<'static, str>, FluentValue<'static>> {
    arguments
        .iter()
        .map(|(key, value)| (Cow::Owned(key.as_ref().to_string()), promote_value(value)))
        .collect()
}

fn promote_value(value: &FluentValue<'_>) -> FluentValue<'static> {
    match value {
        FluentValue::String(text) => FluentValue::String(Cow::Owned(text.as_ref().to_string())),
        FluentValue::Number(number) => FluentValue::Number(number.clone()),
        FluentValue::Custom(custom) => FluentValue::Custom(custom.duplicate()),
        FluentValue::None => FluentValue::None,
        FluentValue::Error => FluentValue::Error,
    }
}
