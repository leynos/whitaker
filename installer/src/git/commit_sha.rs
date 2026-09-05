//! Full Git commit object IDs returned by repository operations.
//!
//! The Git adapter constructs this type only after `git rev-parse` has
//! resolved a commit-ish to a complete object ID. Callers retain the typed
//! value through checkout and prebuilt provenance validation.

use std::fmt;
use thiserror::Error;

/// A validated, full lowercase hexadecimal Git commit object ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitSha(String);

/// An error returned when a value is not a full Git commit object ID.
#[derive(Debug, Error)]
#[error("invalid full Git commit SHA: {reason}")]
pub struct CommitShaError {
    reason: &'static str,
}

impl CommitSha {
    /// Returns the full commit object ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for CommitSha {
    type Error = CommitShaError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() != 40 {
            return Err(CommitShaError {
                reason: "SHA must contain exactly 40 characters",
            });
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(CommitShaError {
                reason: "SHA must contain only lowercase hexadecimal characters",
            });
        }
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Display for CommitSha {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    //! Validates `CommitSha` parsing and rejection behaviour.

    use super::CommitSha;
    use rstest::rstest;

    #[rstest]
    #[case::valid("abc12340000000000000000000000000000000ab", true)]
    #[case::too_short("abc1234", false)]
    #[case::uppercase("ABC12340000000000000000000000000000000AB", false)]
    #[case::non_hex("abc123g0000000000000000000000000000000000", false)]
    fn validates_full_lowercase_hex_shas(#[case] value: &str, #[case] expected: bool) {
        let result = CommitSha::try_from(value);

        assert_eq!(result.is_ok(), expected);
    }
}
