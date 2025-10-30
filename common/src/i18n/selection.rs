use std::fmt;

use super::{Localiser, supports_locale};

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

/// A locale candidate that failed validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocaleRejection {
    source: LocaleSource,
    value: String,
}

impl LocaleRejection {
    /// Construct a new rejection for the provided `source` and `value`.
    #[must_use]
    pub fn new(source: LocaleSource, value: impl Into<String>) -> Self {
        Self {
            source,
            value: value.into(),
        }
    }

    /// Returns the source that provided the rejected locale.
    #[must_use]
    pub fn source(&self) -> LocaleSource {
        self.source
    }

    /// Returns the rejected locale value.
    #[must_use]
    pub fn value(&self) -> &str {
        self.value.as_str()
    }
}

/// Outcome of locale resolution including the effective localiser and provenance.
#[derive(Clone, Debug)]
pub struct LocaleResolution {
    localiser: Localiser,
    source: LocaleSource,
    requested: Option<String>,
    rejections: Vec<LocaleRejection>,
}

impl LocaleResolution {
    fn new(
        localiser: Localiser,
        source: LocaleSource,
        requested: Option<String>,
        rejections: Vec<LocaleRejection>,
    ) -> Self {
        Self {
            localiser,
            source,
            requested,
            rejections,
        }
    }

    /// Returns the effective locale source.
    #[must_use]
    pub fn source(&self) -> LocaleSource {
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
        self.localiser.locale()
    }

    /// Whether the fallback locale was used.
    #[must_use]
    pub fn used_fallback(&self) -> bool {
        self.localiser.used_fallback()
    }

    /// Returns the resolved [`Localiser`].
    #[must_use]
    pub fn localiser(&self) -> &Localiser {
        &self.localiser
    }

    /// Consumes the resolution, yielding the [`Localiser`].
    #[must_use]
    pub fn into_localiser(self) -> Localiser {
        self.localiser
    }

    /// Returns rejected candidates encountered while resolving the locale.
    #[must_use]
    pub fn rejections(&self) -> &[LocaleRejection] {
        self.rejections.as_slice()
    }
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
pub fn resolve_localiser(
    explicit: Option<&str>,
    environment: Option<String>,
    configuration: Option<&str>,
) -> LocaleResolution {
    let mut rejections = Vec::new();

    if let Some(candidate) = normalise(explicit) {
        if supports_locale(candidate.as_str()) {
            return LocaleResolution::new(
                Localiser::new(Some(candidate.as_str())),
                LocaleSource::ExplicitArgument,
                Some(candidate),
                rejections,
            );
        }

        rejections.push(LocaleRejection::new(
            LocaleSource::ExplicitArgument,
            candidate,
        ));
    }

    if let Some(candidate) = normalise(environment.as_deref()) {
        if supports_locale(candidate.as_str()) {
            return LocaleResolution::new(
                Localiser::new(Some(candidate.as_str())),
                LocaleSource::EnvironmentVariable,
                Some(candidate),
                rejections,
            );
        }

        rejections.push(LocaleRejection::new(
            LocaleSource::EnvironmentVariable,
            candidate,
        ));
    }

    if let Some(candidate) = normalise(configuration) {
        if supports_locale(candidate.as_str()) {
            return LocaleResolution::new(
                Localiser::new(Some(candidate.as_str())),
                LocaleSource::Configuration,
                Some(candidate),
                rejections,
            );
        }

        rejections.push(LocaleRejection::new(LocaleSource::Configuration, candidate));
    }

    LocaleResolution::new(
        Localiser::new(None),
        LocaleSource::Fallback,
        None,
        rejections,
    )
}

fn normalise(input: Option<&str>) -> Option<String> {
    input.map(str::trim).and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(None, None, None, LocaleSource::Fallback, "en-GB", true)]
    #[case(Some("cy"), None, None, LocaleSource::ExplicitArgument, "cy", false)]
    #[case(
        None,
        Some(String::from("gd")),
        None,
        LocaleSource::EnvironmentVariable,
        "gd",
        false
    )]
    #[case(None, None, Some("cy"), LocaleSource::Configuration, "cy", false)]
    fn resolves_sources(
        #[case] explicit: Option<&str>,
        #[case] environment: Option<String>,
        #[case] configuration: Option<&str>,
        #[case] expected_source: LocaleSource,
        #[case] expected_locale: &str,
        #[case] expected_fallback: bool,
    ) {
        let resolution = resolve_localiser(explicit, environment, configuration);

        assert_eq!(resolution.source(), expected_source);
        assert_eq!(resolution.locale(), expected_locale);
        assert_eq!(resolution.used_fallback(), expected_fallback);
    }

    #[rstest]
    fn records_rejections_for_invalid_candidates() {
        let resolution = resolve_localiser(Some("zz"), Some(String::from("yy")), Some("xx"));

        let rejections = resolution.rejections();
        assert_eq!(rejections.len(), 3);
        assert!(
            rejections
                .iter()
                .any(|rejection| rejection.source() == LocaleSource::ExplicitArgument)
        );
        assert!(
            rejections
                .iter()
                .any(|rejection| rejection.source() == LocaleSource::EnvironmentVariable)
        );
        assert!(
            rejections
                .iter()
                .any(|rejection| rejection.source() == LocaleSource::Configuration)
        );
    }
}
