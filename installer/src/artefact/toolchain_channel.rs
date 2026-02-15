//! Toolchain channel newtype for artefact naming.
//!
//! Validates that the channel string is non-empty and contains only
//! ASCII alphanumeric characters, hyphens, dots, and underscores — the
//! characters permitted in Rust toolchain channel specifiers, including
//! host-qualified names such as `nightly-2025-09-18-x86_64-unknown-linux-gnu`.

use super::error::{ArtefactError, Result};
use serde::Serialize;
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ToolchainChannel(String);

/// Check that every byte is ASCII alphanumeric, a hyphen, a dot, or an
/// underscore.  Underscores appear in host-qualified toolchain names
/// (e.g. `nightly-2025-09-18-x86_64-unknown-linux-gnu`).
fn is_valid_channel_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_'
}

impl ToolchainChannel {
    /// Return the channel as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;
    ///
    /// let channel: ToolchainChannel = "stable"
    ///     .try_into()
    ///     .expect("valid toolchain channel");
    /// assert_eq!(channel.as_str(), "stable");
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
    /// use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;
    ///
    /// let channel: ToolchainChannel = "nightly-2025-09-18"
    ///     .try_into()
    ///     .expect("valid toolchain channel");
    /// let inner: String = channel.into_inner();
    /// assert_eq!(inner, "nightly-2025-09-18");
    /// ```
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
    use rstest::rstest;

    #[rstest]
    #[case::nightly_with_date("nightly-2025-09-18")]
    #[case::stable("stable")]
    #[case::version_with_dots("1.75.0")]
    #[case::host_qualified("nightly-2025-09-18-x86_64-unknown-linux-gnu")]
    fn accepts_valid_channel(#[case] input: &str) {
        let ch = ToolchainChannel::try_from(input).expect("expected valid channel");
        assert_eq!(ch.as_str(), input);
    }

    #[rstest]
    #[case::empty("", "empty")]
    #[case::whitespace("nightly 2025", "whitespace")]
    #[case::slashes("nightly/latest", "slashes")]
    fn rejects_invalid_channel(#[case] input: &str, #[case] label: &str) {
        let err =
            ToolchainChannel::try_from(input).expect_err("expected rejection of invalid channel");
        assert!(
            matches!(err, ArtefactError::InvalidToolchainChannel { .. }),
            "expected InvalidToolchainChannel for {label}, got {err:?}"
        );
    }

    #[test]
    fn display_shows_inner_value() {
        let ch = ToolchainChannel::try_from("nightly-2025-09-18").expect("known good");
        assert_eq!(format!("{ch}"), "nightly-2025-09-18");
    }

    #[test]
    fn from_owned_string_accepts_valid() {
        let ch = ToolchainChannel::try_from(String::from("nightly-2025-09-18"));
        assert!(ch.is_ok());
    }
}
