//! Target triple validation for prebuilt artefact distribution.
//!
//! Only the five triples listed in ADR-001 are accepted. Any other triple
//! is rejected at construction time with a descriptive error.

use super::error::{ArtefactError, Result};
use serde::Serialize;
use std::fmt;

/// The supported target triples for prebuilt artefact distribution.
///
/// These correspond to the target matrix defined in ADR-001.
const SUPPORTED_TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

/// A validated target triple from the ADR-001 supported set.
///
/// Construction via [`TryFrom`] rejects any triple not in the supported set.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::target::TargetTriple;
///
/// let triple: TargetTriple = "x86_64-unknown-linux-gnu"
///     .try_into()
///     .expect("valid target triple");
/// assert_eq!(triple.as_str(), "x86_64-unknown-linux-gnu");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct TargetTriple(String);

impl TargetTriple {
    /// Return the triple as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Return the full list of supported target triples.
    #[must_use]
    pub fn supported() -> &'static [&'static str] {
        SUPPORTED_TARGETS
    }

    /// Return the shared library extension for this target triple.
    ///
    /// Unlike `builder::library_extension()` which uses compile-time
    /// `#[cfg(target_os)]`, this inspects the target string at runtime,
    /// making it suitable for cross-compilation.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::target::TargetTriple;
    ///
    /// let linux: TargetTriple = "x86_64-unknown-linux-gnu"
    ///     .try_into().expect("valid");
    /// assert_eq!(linux.library_extension(), ".so");
    /// ```
    #[must_use]
    pub fn library_extension(&self) -> &'static str {
        if self.is_windows() {
            ".dll"
        } else if self.is_darwin() {
            ".dylib"
        } else {
            ".so"
        }
    }

    /// Return the library filename prefix for this target triple.
    ///
    /// On Windows, shared libraries have no `lib` prefix; on all other
    /// supported platforms they do.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::target::TargetTriple;
    ///
    /// let win: TargetTriple = "x86_64-pc-windows-msvc"
    ///     .try_into().expect("valid");
    /// assert_eq!(win.library_prefix(), "");
    /// ```
    #[must_use]
    pub fn library_prefix(&self) -> &'static str {
        if self.is_windows() { "" } else { "lib" }
    }

    /// Whether this target is a Windows platform.
    fn is_windows(&self) -> bool {
        self.0.contains("windows")
    }

    /// Whether this target is a macOS (Darwin) platform.
    fn is_darwin(&self) -> bool {
        self.0.contains("darwin")
    }
}

impl TryFrom<&str> for TargetTriple {
    type Error = ArtefactError;

    fn try_from(value: &str) -> Result<Self> {
        if SUPPORTED_TARGETS.contains(&value) {
            Ok(Self(value.to_owned()))
        } else {
            Err(ArtefactError::UnsupportedTarget {
                value: value.to_owned(),
                expected: SUPPORTED_TARGETS.join(", "),
            })
        }
    }
}

impl TryFrom<String> for TargetTriple {
    type Error = ArtefactError;

    fn try_from(value: String) -> Result<Self> {
        Self::try_from(value.as_str())
    }
}

impl AsRef<str> for TargetTriple {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn accepts_all_supported_targets() {
        for target in SUPPORTED_TARGETS {
            let triple = TargetTriple::try_from(*target);
            assert!(triple.is_ok(), "expected {target} to be accepted");
            assert_eq!(triple.expect("checked above").as_str(), *target);
        }
    }

    #[test]
    fn rejects_unsupported_target() {
        let result = TargetTriple::try_from("wasm32-unknown-unknown");
        assert!(result.is_err());
        let err = result.expect_err("expected rejection of unsupported target");
        assert!(
            matches!(err, ArtefactError::UnsupportedTarget { .. }),
            "expected UnsupportedTarget, got {err:?}"
        );
    }

    #[test]
    fn rejects_empty_string() {
        let result = TargetTriple::try_from("");
        assert!(result.is_err());
    }

    #[test]
    fn display_shows_inner_value() {
        let triple = TargetTriple::try_from("aarch64-apple-darwin").expect("known good");
        assert_eq!(format!("{triple}"), "aarch64-apple-darwin");
    }

    #[test]
    fn from_owned_string_accepts_valid() {
        let triple = TargetTriple::try_from(String::from("x86_64-pc-windows-msvc"));
        assert!(triple.is_ok());
    }

    #[test]
    fn from_owned_string_rejects_invalid() {
        let result = TargetTriple::try_from(String::from("mips-unknown-linux"));
        assert!(result.is_err());
    }

    #[test]
    fn supported_returns_all_five_targets() {
        let supported = TargetTriple::supported();
        assert_eq!(supported.len(), 5);
    }

    #[rstest]
    #[case::linux_x86("x86_64-unknown-linux-gnu", ".so", "lib")]
    #[case::linux_arm("aarch64-unknown-linux-gnu", ".so", "lib")]
    #[case::macos_x86("x86_64-apple-darwin", ".dylib", "lib")]
    #[case::macos_arm("aarch64-apple-darwin", ".dylib", "lib")]
    #[case::windows("x86_64-pc-windows-msvc", ".dll", "")]
    fn library_naming_for_target(#[case] triple: &str, #[case] ext: &str, #[case] prefix: &str) {
        let t = TargetTriple::try_from(triple).expect("valid");
        assert_eq!(t.library_extension(), ext);
        assert_eq!(t.library_prefix(), prefix);
    }
}
