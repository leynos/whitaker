//! Resolve localised diagnostic strings (primary/note/help) from Fluent bundles.
//!
//! # Examples
//!
//! ```
//! # use whitaker_common::i18n::{
//! #     Arguments, Localiser, MessageKey, resolve_message_set,
//! # };
//! # fn demo(localiser: &Localiser) -> Result<(), whitaker_common::i18n::I18nError> {
//! #     let args = Arguments::default();
//! let messages = resolve_message_set(
//!     localiser,
//!     MessageKey::new("my-lint.message"),
//!     &args,
//! )?;
//! #     assert!(!messages.primary().is_empty());
//! #     Ok(())
//! # }
//! ```

use super::{Arguments, I18nError, Localiser};
use std::fmt;

/// Identifier for a Fluent message within a localisation bundle.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MessageKey<'a>(&'a str);

impl<'a> MessageKey<'a> {
    /// Construct a new message key wrapper.
    #[must_use]
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }
}

impl<'a> AsRef<str> for MessageKey<'a> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl fmt::Display for MessageKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Identifier for a Fluent attribute attached to a message.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AttrKey<'a>(&'a str);

impl<'a> AttrKey<'a> {
    /// Construct a new attribute key wrapper.
    #[must_use]
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }
}

impl<'a> AsRef<str> for AttrKey<'a> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl fmt::Display for AttrKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Lookup trait used by lint crates to resolve translated diagnostic strings.
pub trait BundleLookup {
    /// Resolve the primary message for `key` using `args`.
    fn message(&self, key: MessageKey<'_>, args: &Arguments<'_>) -> Result<String, I18nError>;

    /// Resolve an attribute message for `key.attribute` using `args`.
    fn attribute(
        &self,
        key: MessageKey<'_>,
        attribute: AttrKey<'_>,
        args: &Arguments<'_>,
    ) -> Result<String, I18nError>;
}

impl BundleLookup for Localiser {
    fn message(&self, key: MessageKey<'_>, args: &Arguments<'_>) -> Result<String, I18nError> {
        self.message_with_args(key.as_ref(), args)
    }

    fn attribute(
        &self,
        key: MessageKey<'_>,
        attribute: AttrKey<'_>,
        args: &Arguments<'_>,
    ) -> Result<String, I18nError> {
        self.attribute_with_args(key.as_ref(), attribute.as_ref(), args)
    }
}

/// Container holding the standard trio of lint diagnostic messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticMessageSet {
    primary: String,
    note: String,
    help: String,
}

impl DiagnosticMessageSet {
    /// Construct a new set of lint diagnostic strings.
    #[must_use]
    ///
    /// # Examples
    ///
    /// ```
    /// # use whitaker_common::i18n::DiagnosticMessageSet;
    /// let messages = DiagnosticMessageSet::new(
    ///     "primary".into(),
    ///     "note".into(),
    ///     "help".into(),
    /// );
    /// assert_eq!(messages.primary(), "primary");
    /// ```
    pub fn new(primary: String, note: String, help: String) -> Self {
        Self {
            primary,
            note,
            help,
        }
    }

    /// Access the primary lint diagnostic.
    #[must_use]
    ///
    /// # Examples
    ///
    /// ```
    /// # use whitaker_common::i18n::DiagnosticMessageSet;
    /// # let messages = DiagnosticMessageSet::new(
    /// #     "primary".into(),
    /// #     "note".into(),
    /// #     "help".into(),
    /// # );
    /// assert_eq!(messages.primary(), "primary");
    /// ```
    pub fn primary(&self) -> &str {
        &self.primary
    }

    /// Access the note attached to the diagnostic.
    #[must_use]
    ///
    /// # Examples
    ///
    /// ```
    /// # use whitaker_common::i18n::DiagnosticMessageSet;
    /// # let messages = DiagnosticMessageSet::new(
    /// #     "primary".into(),
    /// #     "note".into(),
    /// #     "help".into(),
    /// # );
    /// assert_eq!(messages.note(), "note");
    /// ```
    pub fn note(&self) -> &str {
        &self.note
    }

    /// Access the help text attached to the diagnostic.
    #[must_use]
    ///
    /// # Examples
    ///
    /// ```
    /// # use whitaker_common::i18n::DiagnosticMessageSet;
    /// # let messages = DiagnosticMessageSet::new(
    /// #     "primary".into(),
    /// #     "note".into(),
    /// #     "help".into(),
    /// # );
    /// assert_eq!(messages.help(), "help");
    /// ```
    pub fn help(&self) -> &str {
        &self.help
    }
}

/// Resolve the primary, note, and help messages for a lint diagnostic.
const NOTE_ATTR: AttrKey<'static> = AttrKey::new("note");
const HELP_ATTR: AttrKey<'static> = AttrKey::new("help");

#[must_use]
pub fn resolve_message_set(
    lookup: &impl BundleLookup,
    key: MessageKey<'_>,
    args: &Arguments<'_>,
) -> Result<DiagnosticMessageSet, I18nError> {
    let primary = lookup.message(key, args)?;
    let note = lookup.attribute(key, NOTE_ATTR, args)?;
    let help = lookup.attribute(key, HELP_ATTR, args)?;

    Ok(DiagnosticMessageSet::new(primary, note, help))
}
