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
        Ok(messages) => messages,
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

            fallback()
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

/// Remove Unicode isolation marks from Fluent-rendered text.
///
/// Fluent inserts [bidirectional isolation marks][bidi] around interpolated
/// variables. While desirable in general, diagnostic snippets frequently render
/// these markers as replacement glyphs, so the helper strips them to keep
/// messages legible.
///
/// [bidi]: https://unicode.org/reports/tr9/#Explicit_Directional_Isolates
#[must_use]
pub fn strip_isolation_marks(value: &str) -> String {
    const LRI: char = '\u{2068}';
    const PDI: char = '\u{2069}';

    if value.chars().any(|ch| ch == LRI || ch == PDI) {
        value
            .chars()
            .filter(|ch| *ch != LRI && *ch != PDI)
            .collect()
    } else {
        value.to_string()
    }
}
