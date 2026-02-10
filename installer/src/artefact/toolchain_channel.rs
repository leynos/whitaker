//! Toolchain channel newtype for artefact naming.
//!
//! Validates that the channel string is non-empty and contains only
//! ASCII alphanumeric characters, hyphens, dots, and underscores — the
//! characters permitted in Rust toolchain channel specifiers, including
//! host-qualified names such as `nightly-2025-09-18-x86_64-unknown-linux-gnu`.

use super::error::{ArtefactError, Result};
use std::fmt;

/// A validated Rust toolchain channel string (e.g. `nightly-2025-09-18`).
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;
///
/// let channel: ToolchainChannel = "nightly-2025-09-18"
///     .try_into()
///     .expect("valid toolchain channel");
/// assert_eq!(channel.as_str(), "nightly-2025-09-18");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolchainChannel(String);

/// Check that every byte is ASCII alphanumeric, a hyphen, a dot, or an
/// underscore.  Underscores appear in host-qualified toolchain names
/// (e.g. `nightly-2025-09-18-x86_64-unknown-linux-gnu`).
fn is_valid_channel_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_'
}

impl ToolchainChannel {
    /// Return the channel as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<&str> for ToolchainChannel {
    type Error = ArtefactError;

    fn try_from(value: &str) -> Result<Self> {
        if value.is_empty() {
            return Err(ArtefactError::InvalidToolchainChannel {
                reason: "channel must not be empty".to_owned(),
            });
        }
        if let Some(bad) = value.chars().find(|c| !is_valid_channel_char(*c)) {
            return Err(ArtefactError::InvalidToolchainChannel {
                reason: format!("invalid character '{bad}' in channel \"{value}\""),
            });
        }
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for ToolchainChannel {
    type Error = ArtefactError;

    fn try_from(value: String) -> Result<Self> {
        // Delegate to the &str implementation for validation.
        let _ = Self::try_from(value.as_str())?;
        Ok(Self(value))
    }
}

impl AsRef<str> for ToolchainChannel {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolchainChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_nightly_with_date() {
        let ch = ToolchainChannel::try_from("nightly-2025-09-18");
        assert!(ch.is_ok());
        assert_eq!(ch.expect("checked above").as_str(), "nightly-2025-09-18");
    }

    #[test]
    fn accepts_stable() {
        let ch = ToolchainChannel::try_from("stable");
        assert!(ch.is_ok());
    }

    #[test]
    fn accepts_version_with_dots() {
        let ch = ToolchainChannel::try_from("1.75.0");
        assert!(ch.is_ok());
    }

    #[test]
    fn rejects_empty_string() {
        let result = ToolchainChannel::try_from("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ArtefactError::InvalidToolchainChannel { .. }),);
    }

    #[test]
    fn rejects_whitespace() {
        let result = ToolchainChannel::try_from("nightly 2025");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_slashes() {
        let result = ToolchainChannel::try_from("nightly/latest");
        assert!(result.is_err());
    }

    #[test]
    fn display_shows_inner_value() {
        let ch = ToolchainChannel::try_from("nightly-2025-09-18").expect("known good");
        assert_eq!(format!("{ch}"), "nightly-2025-09-18");
    }

    #[test]
    fn accepts_host_qualified_channel_with_underscores() {
        let ch = ToolchainChannel::try_from("nightly-2025-09-18-x86_64-unknown-linux-gnu");
        assert!(ch.is_ok());
    }

    #[test]
    fn from_owned_string_accepts_valid() {
        let ch = ToolchainChannel::try_from(String::from("nightly-2025-09-18"));
        assert!(ch.is_ok());
    }
}
