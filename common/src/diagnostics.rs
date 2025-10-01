//! Ergonomic builders for lint diagnostics and suggestions.

use crate::span::SourceSpan;

/// Applicability of a suggestion, mirroring rustc semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    /// The suggestion can be applied mechanically.
    MachineApplicable,
    /// The suggestion is likely correct but not guaranteed.
    MaybeIncorrect,
    /// The suggestion contains placeholders requiring manual edits.
    HasPlaceholders,
    /// Applicability is not specified.
    Unspecified,
}

/// Represents a fix-it suggestion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    message: String,
    replacement: String,
    applicability: Applicability,
}

impl Suggestion {
    /// Creates a new suggestion.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::diagnostics::{Applicability, Suggestion};
    ///
    /// let suggestion = Suggestion::new("Use expect", "expect(...)".into(), Applicability::MaybeIncorrect);
    /// assert_eq!(suggestion.message(), "Use expect");
    /// ```
    #[must_use]
    pub fn new(
        message: impl Into<String>,
        replacement: String,
        applicability: Applicability,
    ) -> Self {
        Self {
            message: message.into(),
            replacement,
            applicability,
        }
    }

    /// Returns the human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the replacement snippet.
    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// Returns the applicability classification.
    #[must_use]
    pub const fn applicability(&self) -> Applicability {
        self.applicability
    }
}

/// Represents a lint diagnostic with optional notes and suggestions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    code: String,
    message: String,
    span: SourceSpan,
    notes: Vec<String>,
    helps: Vec<String>,
    suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// Returns the lint code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the primary message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the primary span.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    /// Returns additional diagnostic notes.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// Returns help messages.
    #[must_use]
    pub fn helps(&self) -> &[String] {
        &self.helps
    }

    /// Returns collected suggestions.
    #[must_use]
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }
}

/// Builder for [`Diagnostic`] instances.
pub struct DiagnosticBuilder {
    diagnostic: Diagnostic,
}

impl DiagnosticBuilder {
    fn new(code: impl Into<String>, message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            diagnostic: Diagnostic {
                code: code.into(),
                message: message.into(),
                span,
                notes: Vec::new(),
                helps: Vec::new(),
                suggestions: Vec::new(),
            },
        }
    }

    /// Adds a note to the diagnostic.
    #[must_use]
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.diagnostic.notes.push(note.into());
        self
    }

    /// Adds a help message to the diagnostic.
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.diagnostic.helps.push(help.into());
        self
    }

    /// Adds a suggestion to the diagnostic.
    #[must_use]
    pub fn suggestion(mut self, suggestion: Suggestion) -> Self {
        self.diagnostic.suggestions.push(suggestion);
        self
    }

    /// Completes the builder and returns the diagnostic.
    #[must_use]
    pub fn build(self) -> Diagnostic {
        self.diagnostic
    }
}

/// Starts building a lint diagnostic for a given span.
///
/// # Examples
///
/// ```
/// use common::diagnostics::{span_lint, Applicability, Suggestion};
/// use common::span::{SourceLocation, SourceSpan};
///
/// let span = SourceSpan::new(SourceLocation::new(1, 0), SourceLocation::new(1, 4)).unwrap();
/// let diagnostic = span_lint("demo", "Example", span)
///     .help("Consider refactoring")
///     .suggestion(Suggestion::new("Use helper", "helper()".into(), Applicability::MaybeIncorrect))
///     .build();
/// assert_eq!(diagnostic.code(), "demo");
/// ```
#[must_use]
pub fn span_lint(
    code: impl Into<String>,
    message: impl Into<String>,
    span: SourceSpan,
) -> DiagnosticBuilder {
    DiagnosticBuilder::new(code, message, span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::{SourceLocation, SourceSpan};
    use rstest::rstest;

    #[rstest]
    fn builds_diagnostic() {
        let span = SourceSpan::new(SourceLocation::new(2, 1), SourceLocation::new(2, 5)).unwrap();
        let diagnostic = span_lint("lint", "Message", span)
            .note("Note")
            .help("Help")
            .suggestion(Suggestion::new(
                "Fix",
                "fix()".into(),
                Applicability::MachineApplicable,
            ))
            .build();

        assert_eq!(diagnostic.code(), "lint");
        assert_eq!(diagnostic.notes(), &[String::from("Note")]);
        assert_eq!(diagnostic.helps(), &[String::from("Help")]);
        assert_eq!(diagnostic.suggestions().len(), 1);
    }
}
