//! Helper functions for localisation workflows.
//!
//! This module provides high-level conveniences for constructing localisers,
//! rendering locale-aware phrases, and safely resolving diagnostic message sets
//! with fallback support.

use std::env;

use log::debug;

use super::{
    Arguments, DiagnosticMessageSet, Localizer, MessageKey, resolve_localizer, resolve_message_set,
};

/// Construct a [`Localizer`] for `lint_name` using workspace configuration.
///
/// The helper inspects `DYLINT_LOCALE` and the optional configuration locale,
/// logging the chosen source before returning the resolved [`Localizer`].
///
/// # Examples
///
/// ```
/// use common::i18n::get_localizer_for_lint;
///
/// let localizer = get_localizer_for_lint("demo-lint", None);
/// assert_eq!(localizer.locale(), "en-GB");
/// ```
#[must_use]
pub fn get_localizer_for_lint(lint_name: &str, configuration_locale: Option<&str>) -> Localizer {
    let environment_locale =
        env::var_os("DYLINT_LOCALE").and_then(|value| value.into_string().ok());
    let selection = resolve_localizer(None, environment_locale, configuration_locale);

    selection.log_outcome(lint_name);
    selection.into_localizer()
}

/// Render a locale-aware description of the number of predicate branches.
///
/// Welsh and Scottish Gaelic require bespoke noun mutations, so the helper
/// centralises those rules for reuse across diagnostics, UI tests, and
/// localisation suites.
///
/// # Examples
///
/// ```
/// use common::i18n::branch_phrase;
///
/// assert_eq!(branch_phrase("en-GB", 2), "2 branches");
/// assert_eq!(branch_phrase("cy", 3), "tair cangen");
/// assert_eq!(branch_phrase("gd", 1), "1 meur");
/// ```
#[must_use]
pub fn branch_phrase(locale: &str, branches: usize) -> String {
    match locale
        .split_once('-')
        .map(|(lang, _)| lang)
        .unwrap_or(locale)
    {
        "cy" => welsh_branch_phrase(branches),
        "gd" => gaelic_branch_phrase(branches),
        _ => english_branch_phrase(branches),
    }
}

fn english_branch_phrase(branches: usize) -> String {
    match branches {
        1 => String::from("1 branch"),
        _ => format!("{branches} branches"),
    }
}

fn gaelic_branch_phrase(branches: usize) -> String {
    match branches {
        1 => String::from("1 meur"),
        _ => format!("{branches} meuran"),
    }
}

fn welsh_branch_phrase(branches: usize) -> String {
    match branches {
        0 => String::from("dim canghennau"),
        1 => String::from("un gangen"),
        2 => String::from("dwy gangen"),
        3 => String::from("tair cangen"),
        6 => String::from("chwe changen"),
        4 | 5 => format!("{branches} cangen"),
        _ => format!("{branches} canghennau"),
    }
}

/// Report localisation failures by discarding the message.
///
/// Use this helper with [`safe_resolve_message_set`] when a lint only needs
/// deterministic fallback strings and does not want to surface missing
/// localisation details as bug reports.
///
/// # Examples
///
/// ```
/// use common::i18n::{
///     Arguments, DiagnosticMessageSet, Localizer, MessageKey, MessageResolution,
///     noop_reporter, safe_resolve_message_set,
/// };
///
/// let localizer = Localizer::new(Some("en-GB"));
/// let args: Arguments<'static> = Arguments::default();
/// let resolution = MessageResolution {
///     lint_name: "demo-lint",
///     key: MessageKey::new("missing-key"),
///     args: &args,
/// };
/// let fallback = DiagnosticMessageSet::new(
///     "Fallback primary".into(),
///     "Fallback note".into(),
///     "Fallback help".into(),
/// );
///
/// let messages = safe_resolve_message_set(&localizer, resolution, noop_reporter, || fallback);
/// assert_eq!(messages.primary(), "Fallback primary");
/// ```
pub fn noop_reporter(_message: String) {}

/// Resolve a diagnostic message set while logging localisation failures.
///
/// When lookups fail the helper invokes the supplied bug reporter, records the
/// failure in the lint's debug log, and returns deterministic fallback
/// messages.
///
/// # Examples
///
/// ```
/// use common::i18n::testing::RecordingEmitter;
/// use common::i18n::{
///     Arguments, DiagnosticMessageSet, Localizer, MessageKey, MessageResolution,
///     safe_resolve_message_set,
/// };
/// use fluent_templates::fluent_bundle::FluentValue;
/// use std::borrow::Cow;
///
/// let mut args: Arguments<'static> = Arguments::default();
/// args.insert(Cow::Borrowed("subject"), FluentValue::from("demo"));
///
/// let resolution = MessageResolution {
///     lint_name: "demo-lint",
///     key: MessageKey::new("missing-key"),
///     args: &args,
/// };
/// let fallback = DiagnosticMessageSet::new(
///     "Fallback primary".into(),
///     "Fallback note".into(),
///     "Fallback help".into(),
/// );
/// let localizer = Localizer::new(Some("en-GB"));
/// let emitter = RecordingEmitter::default();
///
/// let messages = safe_resolve_message_set(
///     &localizer,
///     resolution,
///     |message| emitter.record(message),
///     || fallback.clone(),
/// );
///
/// assert_eq!(messages.primary(), "Fallback primary");
/// let recorded = emitter.recorded_messages();
/// assert!(recorded[0].contains("missing-key"));
/// ```
#[must_use]
pub fn safe_resolve_message_set(
    localizer: &Localizer,
    resolution: MessageResolution<'_>,
    report_bug: impl FnOnce(String),
    fallback: impl FnOnce() -> DiagnosticMessageSet,
) -> DiagnosticMessageSet {
    match resolve_message_set(localizer, resolution.key, resolution.args) {
        Ok(messages) => messages.strip_isolating_marks(),
        Err(error) => {
            debug!(
                target: resolution.lint_name,
                "localisation error for key `{}` in locale `{}`: {error}; using fallback",
                resolution.key,
                localizer.locale(),
            );

            report_bug(format!(
                "Localisation error for `{}` key `{}` in locale `{}`: {error}",
                resolution.lint_name,
                resolution.key,
                localizer.locale(),
            ));

            fallback().strip_isolating_marks()
        }
    }
}

/// Parameters supplied to [`safe_resolve_message_set`].
#[derive(Clone, Copy)]
pub struct MessageResolution<'a> {
    /// Target lint identifier used for logging and error context.
    pub lint_name: &'a str,
    /// Fluent message key describing the diagnostic entry point.
    pub key: MessageKey<'a>,
    /// Fluent argument map supplied to the lookup.
    pub args: &'a Arguments<'a>,
}

#[cfg(test)]
mod tests {
    use super::branch_phrase;
    use crate::i18n::FALLBACK_LOCALE;

    #[test]
    fn renders_english_branch_phrase() {
        assert_eq!(branch_phrase(FALLBACK_LOCALE, 0), "0 branches");
        assert_eq!(branch_phrase(FALLBACK_LOCALE, 1), "1 branch");
        assert_eq!(branch_phrase(FALLBACK_LOCALE, 4), "4 branches");
    }

    #[test]
    fn renders_gaelic_branch_phrase() {
        assert_eq!(branch_phrase("gd", 1), "1 meur");
        assert_eq!(branch_phrase("gd", 2), "2 meuran");
        assert_eq!(branch_phrase("gd", 3), "3 meuran");
    }

    #[test]
    fn renders_welsh_branch_phrase() {
        assert_eq!(branch_phrase("cy", 0), "dim canghennau");
        assert_eq!(branch_phrase("cy", 1), "un gangen");
        assert_eq!(branch_phrase("cy", 2), "dwy gangen");
        assert_eq!(branch_phrase("cy", 3), "tair cangen");
        assert_eq!(branch_phrase("cy", 6), "chwe changen");
        assert_eq!(branch_phrase("cy", 11), "11 canghennau");
    }
}
