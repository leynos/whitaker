//! SHA-256 digest newtype for artefact verification.
//!
//! Validates that the value is a 64-character lowercase hexadecimal string
//! representing a 256-bit hash digest.

use super::error::{ArtefactError, Result};
use std::fmt;

/// Expected length of a hex-encoded SHA-256 digest.
const DIGEST_HEX_LEN: usize = 64;

/// A validated hex-encoded SHA-256 digest string.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::sha256_digest::Sha256Digest;
///
/// let hex = "a".repeat(64);
/// let digest: Sha256Digest = hex.as_str().try_into().expect("valid SHA-256 digest");
/// assert_eq!(digest.as_str().len(), 64);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Return the digest as a hex string slice.
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

impl TryFrom<&str> for Sha256Digest {
    type Error = ArtefactError;

    fn try_from(value: &str) -> Result<Self> {
        validate_sha256(value)?;
        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for Sha256Digest {
    type Error = ArtefactError;

    fn try_from(value: String) -> Result<Self> {
        // Delegate to the &str implementation for validation.
        let _ = Self::try_from(value.as_str())?;
        Ok(Self(value))
    }
}

impl AsRef<str> for Sha256Digest {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Validate that `value` is a well-formed hex-encoded SHA-256 digest.
fn validate_sha256(value: &str) -> Result<()> {
    if value.len() != DIGEST_HEX_LEN {
        return Err(ArtefactError::InvalidSha256Digest {
            reason: format!(
                "expected {DIGEST_HEX_LEN} hex characters, got {}",
                value.len()
            ),
        });
    }
    if let Some(bad) = value
        .chars()
        .find(|c| !c.is_ascii_hexdigit() || c.is_ascii_uppercase())
    {
        let reason = if bad.is_ascii_uppercase() {
            "digest must be lowercase".to_owned()
        } else {
            format!("non-hex character '{bad}'")
        };
        return Err(ArtefactError::InvalidSha256Digest { reason });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn valid_digest() -> String {
        "a".repeat(64)
    }

    #[rstest]
    fn accepts_valid_sixty_four_char_hex(valid_digest: String) {
        let digest = Sha256Digest::try_from(valid_digest.as_str());
        assert!(digest.is_ok());
    }

    #[rstest]
    fn rejects_too_short() {
        let result = Sha256Digest::try_from("abcdef");
        assert!(result.is_err());
    }

    #[rstest]
    fn rejects_too_long() {
        let long = "a".repeat(65);
        let result = Sha256Digest::try_from(long.as_str());
        assert!(result.is_err());
    }

    #[rstest]
    fn rejects_non_hex_characters() {
        let mut bad = "a".repeat(63);
        bad.push('g');
        let result = Sha256Digest::try_from(bad.as_str());
        assert!(result.is_err());
    }

    #[rstest]
    fn rejects_uppercase_hex() {
        let bad = "A".repeat(64);
        let result = Sha256Digest::try_from(bad.as_str());
        assert!(result.is_err());
    }

    #[rstest]
    fn display_shows_full_digest(valid_digest: String) {
        let digest = Sha256Digest::try_from(valid_digest.as_str()).expect("known good");
        assert_eq!(format!("{digest}"), valid_digest);
    }

    #[rstest]
    fn from_owned_string_accepts_valid(valid_digest: String) {
        let digest = Sha256Digest::try_from(valid_digest);
        assert!(digest.is_ok());
    }
}
