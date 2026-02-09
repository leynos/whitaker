//! Schema version newtype for manifest versioning.
//!
//! Restricts the version to the range `1..=CURRENT_MAX`, matching the
//! versioning policy defined in ADR-001.

use super::error::{ArtefactError, Result};
use std::fmt;

/// The highest schema version this build can read.
const CURRENT_MAX: u32 = 1;

/// A validated manifest schema version.
///
/// The manifest schema is versioned as described in ADR-001. Additive changes
/// increment `schema_version` while keeping backward compatibility; breaking
/// changes require a new installer release that reads both old and new versions
/// during the transition.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::schema_version::SchemaVersion;
///
/// let v = SchemaVersion::current();
/// assert_eq!(u32::from(v), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SchemaVersion(u32);

impl SchemaVersion {
    /// Return the current (latest) schema version.
    #[must_use]
    pub fn current() -> Self {
        Self(CURRENT_MAX)
    }

    /// Return the inner version number.
    #[must_use]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for SchemaVersion {
    type Error = ArtefactError;

    fn try_from(value: u32) -> Result<Self> {
        if value == 0 || value > CURRENT_MAX {
            return Err(ArtefactError::UnsupportedSchemaVersion {
                value,
                max: CURRENT_MAX,
            });
        }
        Ok(Self(value))
    }
}

impl From<SchemaVersion> for u32 {
    fn from(v: SchemaVersion) -> Self {
        v.0
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_returns_version_one() {
        let v = SchemaVersion::current();
        assert_eq!(v.as_u32(), 1);
    }

    #[test]
    fn accepts_version_one() {
        let v = SchemaVersion::try_from(1_u32);
        assert!(v.is_ok());
        assert_eq!(v.expect("checked above").as_u32(), 1);
    }

    #[test]
    fn rejects_version_zero() {
        let result = SchemaVersion::try_from(0_u32);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ArtefactError::UnsupportedSchemaVersion { value: 0, max: 1 }
        ));
    }

    #[test]
    fn rejects_version_above_max() {
        let result = SchemaVersion::try_from(2_u32);
        assert!(result.is_err());
    }

    #[test]
    fn into_u32_round_trips() {
        let v = SchemaVersion::current();
        let n: u32 = v.into();
        assert_eq!(n, 1);
    }

    #[test]
    fn display_shows_number() {
        let v = SchemaVersion::current();
        assert_eq!(format!("{v}"), "1");
    }
}
