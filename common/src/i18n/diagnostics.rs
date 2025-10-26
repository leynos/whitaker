use super::{Arguments, I18nError, Localiser};

/// Lookup trait used by lint crates to resolve translated diagnostic strings.
pub trait BundleLookup {
    /// Resolve the primary message for `key` using `args`.
    fn message(&self, key: &str, args: &Arguments<'_>) -> Result<String, I18nError>;

    /// Resolve an attribute message for `key.attribute` using `args`.
    fn attribute(
        &self,
        key: &str,
        attribute: &str,
        args: &Arguments<'_>,
    ) -> Result<String, I18nError>;
}

impl BundleLookup for Localiser {
    fn message(&self, key: &str, args: &Arguments<'_>) -> Result<String, I18nError> {
        self.message_with_args(key, args)
    }

    fn attribute(
        &self,
        key: &str,
        attribute: &str,
        args: &Arguments<'_>,
    ) -> Result<String, I18nError> {
        self.attribute_with_args(key, attribute, args)
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
    pub fn new(primary: String, note: String, help: String) -> Self {
        Self {
            primary,
            note,
            help,
        }
    }

    /// Access the primary lint diagnostic.
    #[must_use]
    pub fn primary(&self) -> &str {
        &self.primary
    }

    /// Access the note attached to the diagnostic.
    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }

    /// Access the help text attached to the diagnostic.
    #[must_use]
    pub fn help(&self) -> &str {
        &self.help
    }
}

/// Resolve the primary, note, and help messages for a lint diagnostic.
pub fn resolve_message_set(
    lookup: &impl BundleLookup,
    key: &str,
    args: &Arguments<'_>,
) -> Result<DiagnosticMessageSet, I18nError> {
    let primary = lookup.message(key, args)?;
    let note = lookup.attribute(key, "note", args)?;
    let help = lookup.attribute(key, "help", args)?;

    Ok(DiagnosticMessageSet::new(primary, note, help))
}
