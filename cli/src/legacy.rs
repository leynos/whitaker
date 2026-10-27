//! Compatibility with the existing installer's source-fallback control.

use std::ffi::OsStr;

/// Translate the source-fallback environment value into a configuration override.
///
/// Disabled values leave lower-priority settings intact; non-Unicode values
/// forbid fallback, matching the installer.
///
/// # Examples
///
/// ```
/// use std::ffi::OsStr;
/// use whitaker_cli::source_fallback_override;
///
/// assert_eq!(source_fallback_override(Some(OsStr::new("true"))), Some(false));
/// assert_eq!(source_fallback_override(Some(OsStr::new("false"))), None);
/// ```
#[must_use]
pub fn source_fallback_override(value: Option<&OsStr>) -> Option<bool> {
    value.and_then(|value| match value.to_str() {
        Some(text)
            if matches!(
                text.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false"
            ) =>
        {
            None
        }
        _ => Some(false),
    })
}

#[cfg(test)]
mod tests {
    use super::source_fallback_override;
    use rstest::rstest;
    use std::ffi::OsStr;

    /// Preserve the installer's interpretation of text values.
    #[rstest]
    #[case(None, None)]
    #[case(Some(""), None)]
    #[case(Some(" 0 "), None)]
    #[case(Some(" FALSE "), None)]
    #[case(Some("true"), Some(false))]
    #[case(Some("anything"), Some(false))]
    fn maps_control(#[case] value: Option<&str>, #[case] expected: Option<bool>) {
        assert_eq!(source_fallback_override(value.map(OsStr::new)), expected);
    }

    /// Preserve the installer's treatment of non-Unicode values.
    #[cfg(unix)]
    #[test]
    fn non_unicode_enables_control() {
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(
            source_fallback_override(Some(OsStr::from_bytes(&[0xff]))),
            Some(false)
        );
    }
}
