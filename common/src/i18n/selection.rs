use std::fmt;

use log::{debug, warn};

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

/// Outcome of locale resolution including the effective localiser and provenance.
#[derive(Clone, Debug)]
pub struct LocaleSelection {
    localiser: Localiser,
    source: LocaleSource,
    requested: Option<String>,
}

impl LocaleSelection {
    const fn new(localiser: Localiser, source: LocaleSource, requested: Option<String>) -> Self {
        Self {
            localiser,
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

    /// Consumes the selection, yielding the [`Localiser`].
    #[must_use]
    pub fn into_localiser(self) -> Localiser {
        self.localiser
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
            Localiser::new(Some(candidate)),
            source,
            Some(candidate.to_owned()),
        ));
    }

    warn!(
        target: "i18n::selection",
        "skipping unsupported {source} `{candidate}`; falling back to en-GB",
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
pub fn resolve_localiser(
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
        .unwrap_or_else(|| LocaleSelection::new(Localiser::new(None), LocaleSource::Fallback, None))
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
    #[case(
        Some("zz"),
        Some(String::from("yy")),
        Some("cy"),
        LocaleSource::Configuration,
        "cy",
        false
    )]
    fn resolves_sources(
        #[case] explicit: Option<&str>,
        #[case] environment: Option<String>,
        #[case] configuration: Option<&str>,
        #[case] expected_source: LocaleSource,
        #[case] expected_locale: &str,
        #[case] expected_fallback: bool,
    ) {
        let selection = resolve_localiser(explicit, environment, configuration);

        assert_eq!(selection.source(), expected_source);
        assert_eq!(selection.locale(), expected_locale);
        assert_eq!(selection.used_fallback(), expected_fallback);
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
