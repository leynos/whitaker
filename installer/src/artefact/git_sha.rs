//! Git SHA newtype for artefact naming.
//!
//! Validates that the value is a non-empty, lowercase hexadecimal string
//! of 7–40 characters, matching the range of abbreviated to full git
//! object names.

use super::error::{ArtefactError, Result};
use serde::Serialize;
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
/// let sha: GitSha = "abc1234".try_into().expect("valid git SHA");
/// assert_eq!(sha.as_str(), "abc1234");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct GitSha(String);

impl GitSha {
    /// Return the SHA as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::git_sha::GitSha;
    ///
    /// let sha: GitSha = "abc1234".try_into().expect("valid git SHA");
    /// assert_eq!(sha.as_str(), "abc1234");
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
    /// use whitaker_installer::artefact::git_sha::GitSha;
    ///
    /// let sha: GitSha = "abc1234".try_into().expect("valid git SHA");
    /// assert_eq!(sha.into_inner(), "abc1234");
    /// ```
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
        // Delegate to the &str implementation for validation.
        let _ = Self::try_from(value.as_str())?;
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

impl<'de> serde::Deserialize<'de> for GitSha {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
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
    if let Some(bad) = value
        .chars()
        .find(|c| !c.is_ascii_hexdigit() || c.is_ascii_uppercase())
    {
        let reason = if !bad.is_ascii_hexdigit() {
            format!("non-hex character '{bad}'")
        } else {
            "SHA must be lowercase".to_owned()
        };
        return Err(ArtefactError::InvalidGitSha {
            value: value.to_owned(),
            reason,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

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

    /// Build an invalid SHA string for the given test case.
    fn invalid_sha(label: &str) -> String {
        match label {
            "empty" => String::new(),
            "too_short" => "abc123".to_owned(),
            "too_long" => "a".repeat(41),
            "non_hex" => "abc123g".to_owned(),
            "uppercase" => "ABC1234".to_owned(),
            other => panic!("unknown case: {other}"),
        }
    }

    #[rstest]
    #[case::empty("empty")]
    #[case::too_short("too_short")]
    #[case::too_long("too_long")]
    #[case::non_hex("non_hex")]
    #[case::uppercase("uppercase")]
    fn rejects_invalid_sha(#[case] label: &str) {
        let input = invalid_sha(label);
        let err = GitSha::try_from(input.as_str()).expect_err("expected rejection of invalid SHA");
        assert!(
            matches!(err, ArtefactError::InvalidGitSha { .. }),
            "expected InvalidGitSha for {label}, got {err:?}"
        );
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

    #[test]
    fn serde_round_trip() {
        let sha = GitSha::try_from("abc1234").expect("valid");
        let json = serde_json::to_string(&sha).expect("serialize");
        let back: GitSha = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sha, back);
    }

    #[test]
    fn deserialize_rejects_invalid() {
        let json = r#""AB""#; // too short + uppercase
        let result: std::result::Result<GitSha, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
