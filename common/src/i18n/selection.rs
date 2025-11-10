//! Locale resolver wiring explicit overrides, environment variables, and
//! configuration before falling back to the bundled localiser.

use std::fmt;

use log::{debug, warn};

use super::{Localizer, supports_locale};

/// Source for a resolved locale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocaleSource {
    /// Locale supplied explicitly by the caller.
    ExplicitArgument,
    /// Locale sourced from the `DYLINT_LOCALE` environment variable.
    EnvironmentVariable,
    /// Locale taken from `dylint.toml` configuration.
    Configuration,
    /// Fallback locale bundled with the Whitaker suite.
    Fallback,
}

impl fmt::Display for LocaleSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExplicitArgument => formatter.write_str("explicit locale override"),
            Self::EnvironmentVariable => formatter.write_str("DYLINT_LOCALE"),
            Self::Configuration => formatter.write_str("configuration locale"),
            Self::Fallback => formatter.write_str("fallback locale"),
        }
    }
}

/// Outcome of locale resolution including the effective localiser and provenance.
#[derive(Clone, Debug)]
pub struct LocaleSelection {
    localizer: Localizer,
    source: LocaleSource,
    requested: Option<String>,
}

impl LocaleSelection {
    const fn new(localizer: Localizer, source: LocaleSource, requested: Option<String>) -> Self {
        Self {
            localizer,
            source,
            requested,
        }
    }

    /// Returns the effective locale source.
    #[must_use]
    pub const fn source(&self) -> LocaleSource {
        self.source
    }

    /// Returns the locale requested by the resolved source, if any.
    #[must_use]
    pub fn requested(&self) -> Option<&str> {
        self.requested.as_deref()
    }

    /// Returns the resolved locale tag.
    #[must_use]
    pub fn locale(&self) -> &str {
        self.localizer.locale()
    }

    /// Whether the fallback locale was used.
    #[must_use]
    pub fn used_fallback(&self) -> bool {
        self.localizer.used_fallback()
    }

    /// Returns the resolved [`Localizer`].
    #[must_use]
    pub fn localizer(&self) -> &Localizer {
        &self.localizer
    }

    /// Consumes the selection, yielding the [`Localizer`].
    #[must_use]
    pub fn into_localizer(self) -> Localizer {
        self.localizer
    }

    /// Emit a debug log summarising the resolved locale.
    pub fn log_outcome(&self, target: &str) {
        debug!(
            target: target,
            "resolved {} to `{}`",
            self.source(),
            self.locale(),
        );
    }
}

/// Attempt to resolve a locale candidate from the given source.
fn try_resolve_candidate(source: LocaleSource, raw: Option<&str>) -> Option<LocaleSelection> {
    let candidate = normalise_locale(raw)?;

    if supports_locale(candidate) {
        return Some(LocaleSelection::new(
            Localizer::new(Some(candidate)),
            source,
            Some(candidate.to_owned()),
        ));
    }

    warn!(
        target: "i18n::selection",
        "skipping unsupported {source} `{candidate}`; continuing locale resolution",
    );

    None
}

/// Resolve a locale using explicit, environment, and configuration overrides.
///
/// The resolver considers candidates in the following order:
///
/// 1. The explicit locale supplied by the caller.
/// 2. The `DYLINT_LOCALE` environment variable.
/// 3. The workspace configuration (`dylint.toml`).
/// 4. The embedded fallback when no candidate is valid.
#[must_use]
pub fn resolve_localizer(
    explicit: Option<&str>,
    environment: Option<String>,
    configuration: Option<&str>,
) -> LocaleSelection {
    let candidates = [
        (LocaleSource::ExplicitArgument, explicit),
        (LocaleSource::EnvironmentVariable, environment.as_deref()),
        (LocaleSource::Configuration, configuration),
    ];

    candidates
        .into_iter()
        .find_map(|(source, raw)| try_resolve_candidate(source, raw))
        .unwrap_or_else(|| LocaleSelection::new(Localizer::new(None), LocaleSource::Fallback, None))
}

/// Trim whitespace and discard empty locale candidates.
#[must_use]
pub fn normalise_locale(input: Option<&str>) -> Option<&str> {
    input
        .map(str::trim)
        .and_then(|value| if value.is_empty() { None } else { Some(value) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[derive(Clone, Copy, Debug)]
    struct ResolutionCase {
        explicit: Option<&'static str>,
        environment: Option<&'static str>,
        configuration: Option<&'static str>,
        expected_source: LocaleSource,
        expected_locale: &'static str,
        expected_fallback: bool,
    }

    impl ResolutionCase {
        fn environment(&self) -> Option<String> {
            self.environment.map(String::from)
        }
    }

    #[rstest]
    #[case(ResolutionCase {
        explicit: None,
        environment: None,
        configuration: None,
        expected_source: LocaleSource::Fallback,
        expected_locale: "en-GB",
        expected_fallback: true,
    })]
    #[case(ResolutionCase {
        explicit: Some("cy"),
        environment: None,
        configuration: None,
        expected_source: LocaleSource::ExplicitArgument,
        expected_locale: "cy",
        expected_fallback: false,
    })]
    #[case(ResolutionCase {
        explicit: None,
        environment: Some("gd"),
        configuration: None,
        expected_source: LocaleSource::EnvironmentVariable,
        expected_locale: "gd",
        expected_fallback: false,
    })]
    #[case(ResolutionCase {
        explicit: None,
        environment: None,
        configuration: Some("cy"),
        expected_source: LocaleSource::Configuration,
        expected_locale: "cy",
        expected_fallback: false,
    })]
    #[case(ResolutionCase {
        explicit: Some("zz"),
        environment: Some("yy"),
        configuration: Some("cy"),
        expected_source: LocaleSource::Configuration,
        expected_locale: "cy",
        expected_fallback: false,
    })]
    fn resolves_sources(#[case] case: ResolutionCase) {
        let selection = resolve_localizer(case.explicit, case.environment(), case.configuration);

        assert_eq!(selection.source(), case.expected_source);
        assert_eq!(selection.locale(), case.expected_locale);
        assert_eq!(selection.used_fallback(), case.expected_fallback);
    }

    #[rstest]
    #[case(None, None)]
    #[case(Some(""), None)]
    #[case(Some("  "), None)]
    #[case(Some("cy"), Some("cy"))]
    #[case(Some(" cy "), Some("cy"))]
    fn normalises_candidates(#[case] input: Option<&str>, #[case] expected: Option<&str>) {
        assert_eq!(normalise_locale(input), expected);
    }
}
