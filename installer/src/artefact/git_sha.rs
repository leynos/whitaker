//! Git SHA newtype for artefact naming.
//!
//! Validates that the value is a non-empty, lowercase hexadecimal string
//! of 7–40 characters, matching the range of abbreviated to full git
//! object names.

use super::error::{ArtefactError, Result};
use std::fmt;

/// Minimum length of an abbreviated git SHA (7 hex characters).
const MIN_LEN: usize = 7;

/// Maximum length of a full git SHA-1 (40 hex characters).
const MAX_LEN: usize = 40;

/// A validated abbreviated or full git commit SHA.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::git_sha::GitSha;
///
/// let sha: GitSha = "abc1234".try_into().unwrap();
/// assert_eq!(sha.as_str(), "abc1234");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitSha(String);

impl GitSha {
    /// Return the SHA as a string slice.
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

impl TryFrom<&str> for GitSha {
    type Error = ArtefactError;

    fn try_from(value: &str) -> Result<Self> {
        validate_git_sha(value)?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for GitSha {
    type Error = ArtefactError;

    fn try_from(value: String) -> Result<Self> {
        validate_git_sha(&value)?;
        Ok(Self(value))
    }
}

impl AsRef<str> for GitSha {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitSha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Validate that `value` is a well-formed git SHA.
fn validate_git_sha(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(ArtefactError::InvalidGitSha {
            value: value.to_owned(),
            reason: "SHA must not be empty".to_owned(),
        });
    }
    if value.len() < MIN_LEN {
        return Err(ArtefactError::InvalidGitSha {
            value: value.to_owned(),
            reason: format!(
                "SHA must be at least {MIN_LEN} characters, got {}",
                value.len()
            ),
        });
    }
    if value.len() > MAX_LEN {
        return Err(ArtefactError::InvalidGitSha {
            value: value.to_owned(),
            reason: format!(
                "SHA must be at most {MAX_LEN} characters, got {}",
                value.len()
            ),
        });
    }
    if let Some(bad) = value.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(ArtefactError::InvalidGitSha {
            value: value.to_owned(),
            reason: format!("non-hex character '{bad}'"),
        });
    }
    if value.chars().any(|c| c.is_ascii_uppercase()) {
        return Err(ArtefactError::InvalidGitSha {
            value: value.to_owned(),
            reason: "SHA must be lowercase".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_seven_char_abbreviated_sha() {
        let sha = GitSha::try_from("abc1234");
        assert!(sha.is_ok());
        assert_eq!(sha.expect("checked above").as_str(), "abc1234");
    }

    #[test]
    fn accepts_full_forty_char_sha() {
        let full = "a".repeat(40);
        let sha = GitSha::try_from(full.as_str());
        assert!(sha.is_ok());
    }

    #[test]
    fn rejects_empty_string() {
        let result = GitSha::try_from("");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_too_short() {
        let result = GitSha::try_from("abc123");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ArtefactError::InvalidGitSha { .. }));
    }

    #[test]
    fn rejects_too_long() {
        let long = "a".repeat(41);
        let result = GitSha::try_from(long.as_str());
        assert!(result.is_err());
    }

    #[test]
    fn rejects_non_hex_characters() {
        let result = GitSha::try_from("abc123g");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_uppercase_hex() {
        let result = GitSha::try_from("ABC1234");
        assert!(result.is_err());
    }

    #[test]
    fn display_shows_inner_value() {
        let sha = GitSha::try_from("deadbeef").expect("known good");
        assert_eq!(format!("{sha}"), "deadbeef");
    }

    #[test]
    fn from_owned_string_accepts_valid() {
        let sha = GitSha::try_from(String::from("abc1234"));
        assert!(sha.is_ok());
    }
}
