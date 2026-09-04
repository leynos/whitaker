//! Suite reference newtype for pinning the lint suite.
//!
//! The installer builds the lint suite from a Whitaker checkout. Without a
//! pin it uses whatever is at the default branch tip, so a change to that
//! branch alters a consumer's lint results with no commit in the consumer.
//! This newtype carries the git reference a consumer asked for, validated
//! before it reaches a `git` command line.
//!
//! Validation is deliberately conservative. A reference here is passed to
//! `git checkout`, so a value beginning with `-` would be read as an option
//! rather than a reference, and the characters git itself forbids in a
//! reference name are rejected rather than left for git to complain about
//! after a clone has already been paid for.

use super::error::{ArtefactError, Result};
use serde::Serialize;
use std::fmt;

/// Longest reference this accepts. Git imposes no such limit, but a value
/// beyond this is a mistake rather than a reference, and an unbounded one
/// would reach a command line.
const MAX_LEN: usize = 250;

/// Characters git refuses in a reference name, per `git check-ref-format`,
/// plus the shell-significant ones that have no business in one.
const FORBIDDEN: [char; 10] = [' ', '~', '^', ':', '?', '*', '[', '\\', '\u{7f}', '\0'];

/// A validated git reference naming the lint suite to build.
///
/// Accepts a tag, a branch, or a commit SHA; the installer hands it to
/// `git checkout` and lets git decide which it is.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::suite_ref::SuiteRef;
///
/// let reference: SuiteRef = "v0.2.7".try_into().expect("valid reference");
/// assert_eq!(reference.as_str(), "v0.2.7");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SuiteRef(String);

impl SuiteRef {
    /// Return the reference as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::suite_ref::SuiteRef;
    ///
    /// let reference: SuiteRef = "main".try_into().expect("valid reference");
    /// assert_eq!(reference.as_str(), "main");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner string.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::suite_ref::SuiteRef;
    ///
    /// let reference: SuiteRef = "v0.2.6".try_into().expect("valid reference");
    /// assert_eq!(reference.into_inner(), "v0.2.6");
    /// ```
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<&str> for SuiteRef {
    type Error = ArtefactError;

    fn try_from(value: &str) -> Result<Self> {
        validate_suite_ref(value)?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for SuiteRef {
    type Error = ArtefactError;

    fn try_from(value: String) -> Result<Self> {
        validate_suite_ref(&value)?;
        Ok(Self(value))
    }
}

impl std::str::FromStr for SuiteRef {
    type Err = ArtefactError;

    /// Parse a reference, so a command-line parser can validate one directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use whitaker_installer::artefact::suite_ref::SuiteRef;
    ///
    /// assert!(SuiteRef::from_str("v0.2.7").is_ok());
    /// assert!(SuiteRef::from_str("--upload-pack=evil").is_err());
    /// ```
    fn from_str(value: &str) -> Result<Self> {
        value.try_into()
    }
}

impl fmt::Display for SuiteRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Reject a reference git would refuse, or that would reach a command line
/// as something other than a reference.
fn validate_suite_ref(value: &str) -> Result<()> {
    let reject = |reason: &str| {
        Err(ArtefactError::InvalidSuiteRef {
            value: value.to_owned(),
            reason: reason.to_owned(),
        })
    };

    if value.is_empty() {
        return reject("must not be empty");
    }
    if value.len() > MAX_LEN {
        return reject(&format!("must be at most {MAX_LEN} characters"));
    }
    // A leading dash reaches `git checkout` as an option rather than as a
    // reference, so it is refused here rather than misread there.
    if value.starts_with('-') {
        return reject("must not begin with '-'");
    }
    if value.starts_with('/') || value.ends_with('/') {
        return reject("must not begin or end with '/'");
    }
    if value.ends_with(".lock") {
        return reject("must not end with '.lock'");
    }
    if value.contains("..") {
        return reject("must not contain '..'");
    }
    if value.contains("//") {
        return reject("must not contain '//'");
    }
    if value.contains("@{") {
        return reject("must not contain '@{'");
    }
    if let Some(found) = value.chars().find(|c| FORBIDDEN.contains(c)) {
        return reject(&format!("must not contain {found:?}"));
    }
    if value.chars().any(char::is_control) {
        return reject("must not contain control characters");
    }
    Ok(())
}

#[cfg(test)]
#[path = "suite_ref_tests.rs"]
mod tests;
