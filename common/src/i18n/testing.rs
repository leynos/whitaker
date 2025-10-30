use std::borrow::Cow;

use super::{Arguments, AttrKey, BundleLookup, I18nError, MessageKey};

/// Test double that always returns `MissingMessage` errors for message lookups.
///
/// Shared across localisation tests to exercise error-handling paths when Fluent
/// bundles do not contain the requested messages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailingLookup {
    message_key: Cow<'static, str>,
    locale: Cow<'static, str>,
}

impl FailingLookup {
    /// Construct a failing lookup for `message_key` using the default test locale.
    #[must_use]
    pub fn new(message_key: impl Into<Cow<'static, str>>) -> Self {
        Self::with_locale(message_key, Cow::Borrowed("test"))
    }

    /// Construct a failing lookup for `message_key` and `locale`.
    #[must_use]
    pub fn with_locale(
        message_key: impl Into<Cow<'static, str>>,
        locale: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            message_key: message_key.into(),
            locale: locale.into(),
        }
    }
}

impl BundleLookup for FailingLookup {
    fn message(&self, _key: MessageKey<'_>, _args: &Arguments<'_>) -> Result<String, I18nError> {
        Err(I18nError::MissingMessage {
            key: self.message_key.clone().into_owned(),
            locale: self.locale.clone().into_owned(),
        })
    }

    fn attribute(
        &self,
        _key: MessageKey<'_>,
        _attribute: AttrKey<'_>,
        _args: &Arguments<'_>,
    ) -> Result<String, I18nError> {
        Err(I18nError::MissingMessage {
            key: self.message_key.clone().into_owned(),
            locale: self.locale.clone().into_owned(),
        })
    }
}
