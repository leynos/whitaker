//! Verification policy for prebuilt artefact integrity.
//!
//! Defines the policy that governs how the installer verifies downloaded
//! artefacts before extraction. The policy is a value type that captures
//! the rules from ADR-001 without performing I/O — actual hash computation
//! and file operations are deferred to later tasks.

use std::fmt;

/// Policy governing how a downloaded artefact is verified before use.
///
/// ADR-001 requires checksum validation before extraction. This type
/// encodes the verification rules as data so that the installer can
/// inspect and report the active policy without side effects.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::verification::VerificationPolicy;
///
/// let policy = VerificationPolicy::default();
/// assert!(policy.require_checksum());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationPolicy {
    require_checksum: bool,
}

impl VerificationPolicy {
    /// Return whether checksum verification is required.
    ///
    /// When true (the default), the installer must compute the SHA-256
    /// digest of the downloaded archive and compare it against the digest
    /// recorded in the manifest before extracting any files.
    #[must_use]
    pub fn require_checksum(&self) -> bool {
        self.require_checksum
    }
}

impl Default for VerificationPolicy {
    /// The default policy requires checksum verification, matching ADR-001.
    fn default() -> Self {
        Self {
            require_checksum: true,
        }
    }
}

impl fmt::Display for VerificationPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.require_checksum {
            write!(f, "checksum verification required")
        } else {
            write!(f, "checksum verification disabled")
        }
    }
}

/// The action to take when artefact verification fails.
///
/// ADR-001 specifies that verification failures must trigger a local
/// build fallback with a user-visible warning. This enum encodes that
/// decision as an inspectable value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum VerificationFailureAction {
    /// Fall back to a local build and emit a warning.
    #[default]
    FallbackWithWarning,
}

impl fmt::Display for VerificationFailureAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FallbackWithWarning => {
                write!(f, "fall back to local build with warning")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_requires_checksum() {
        let policy = VerificationPolicy::default();
        assert!(policy.require_checksum());
    }

    #[test]
    fn policy_display_when_required() {
        let policy = VerificationPolicy::default();
        assert_eq!(format!("{policy}"), "checksum verification required");
    }

    #[test]
    fn default_failure_action_is_fallback_with_warning() {
        let action = VerificationFailureAction::default();
        assert_eq!(action, VerificationFailureAction::FallbackWithWarning);
    }

    #[test]
    fn failure_action_display() {
        let action = VerificationFailureAction::FallbackWithWarning;
        assert_eq!(format!("{action}"), "fall back to local build with warning");
    }
}
