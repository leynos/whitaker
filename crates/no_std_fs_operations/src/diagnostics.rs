//! Localized diagnostics for the `no_std_fs_operations` lint.

use std::borrow::Cow;

use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;
use whitaker_common::i18n::{
    Arguments,
    DiagnosticMessageSet,
    FluentValue,
    Localizer,
    MessageKey,
    MessageResolution,
    noop_reporter,
    safe_resolve_message_set,
};
#[cfg(test)]
use whitaker_common::i18n::{BundleLookup, I18nError, resolve_message_set};

use crate::{NO_STD_FS_OPERATIONS, usage::StdFsUsage};

/// Emit a diagnostic for a detected `std::fs` usage.
pub(crate) fn emit_diagnostic(
    cx: &LateContext<'_>,
    span: Span,
    usage: &StdFsUsage,
    localizer: &Localizer,
) {
    let messages = resolve_messages(localizer, usage.operation(), noop_reporter);

    cx.emit_span_lint(
        NO_STD_FS_OPERATIONS,
        span,
        rustc_lint::errors::DiagDecorator(move |lint| {
            lint.primary_message(sanitize_message(messages.primary()));
            lint.note(sanitize_message(messages.note()));
            lint.help(sanitize_message(messages.help()));
        }),
    );
}

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("no_std_fs_operations");

/// Resolve the diagnostic messages for `operation`, as the lint emits them.
///
/// A lookup failure is passed to `report_bug` and replaced by the built-in
/// English messages. Production passes `noop_reporter`; tests pass a recorder
/// so a locale that silently falls back is caught.
pub(crate) fn resolve_messages(
    localizer: &Localizer,
    operation: &str,
    report_bug: impl FnOnce(String),
) -> StdFsMessages {
    let args = message_arguments(operation);
    let resolution = MessageResolution {
        lint_name: "no_std_fs_operations",
        key: MESSAGE_KEY,
        args: &args,
    };
    safe_resolve_message_set(localizer, resolution, report_bug, || {
        fallback_messages(operation)
    })
}

/// Build the Fluent arguments every lookup of this lint's message uses.
fn message_arguments(operation: &str) -> Arguments<'static> {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("operation"),
        FluentValue::from(operation.to_owned()),
    );
    args
}

pub(crate) type StdFsMessages = DiagnosticMessageSet;

/// Build default diagnostic messages when localization lookup fails.
fn fallback_messages(operation: &str) -> StdFsMessages {
    let primary = format!(
        "Avoid using std::fs operation `{operation}`; require capability-bearing handles instead."
    );
    let note = concat!(
        "std::fs reads the ambient working directory, ",
        "so it bypasses the capability model enforced by cap-std and camino."
    )
    .to_owned();
    let help = concat!(
        "Pass `cap_std::fs::Dir` handles and camino::Utf8Path/Utf8PathBuf arguments down to the \
         call ",
        "so only explicit capabilities touch the filesystem."
    )
    .to_owned();
    DiagnosticMessageSet::new(primary, note, help)
}

/// Remove directional isolation marks before embedding localized text.
fn sanitize_message(text: &str) -> String {
    text.chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}'))
        .collect()
}

/// Build localized diagnostics for the supplied filesystem operation.
#[cfg(test)]
pub(crate) fn localized_messages(
    lookup: &impl BundleLookup,
    operation: &str,
) -> Result<StdFsMessages, I18nError> {
    resolve_message_set(lookup, MESSAGE_KEY, &message_arguments(operation))
}

#[cfg(test)]
mod tests {
    //! Tests for diagnostic message sanitization and per-locale resolution.

    use whitaker_common::i18n::{
        FALLBACK_LOCALE,
        Localizer,
        available_locales,
        normalize_fluent_output,
        testing::RecordingEmitter,
    };

    use super::{StdFsMessages, localized_messages, resolve_messages, sanitize_message};

    /// The operation every locale renders into its messages.
    const OPERATION: &str = "std::fs::read";

    /// Resolve the messages for `locale` through the lint's production path,
    /// failing if the lookup falls back to the built-in English messages.
    fn production_messages(locale: &str) -> StdFsMessages {
        let localizer = Localizer::new(Some(locale));
        assert!(
            !localizer.used_fallback(),
            "`{locale}` should load its own catalogue"
        );
        let reports = RecordingEmitter::default();
        let messages = resolve_messages(&localizer, OPERATION, |report| reports.record(report));
        assert!(
            reports.recorded_messages().is_empty(),
            "`{locale}` fell back to the built-in messages: {:?}",
            reports.recorded_messages()
        );
        messages
    }

    /// Check that every shipped locale resolves this lint's messages through
    /// the path `emit_diagnostic` uses, keeps its frozen tokens, and is not a
    /// copy of the English source. The `ui-*` fixtures cover only some locales
    /// through the compiler; this covers the rest without a compiler run.
    #[test]
    fn every_locale_renders_the_lint_messages() {
        let english = production_messages(FALLBACK_LOCALE);
        for locale in available_locales() {
            let messages = production_messages(locale);
            assert!(
                messages.primary().contains(OPERATION),
                "`{locale}` primary message lost the operation: {}",
                messages.primary()
            );
            assert!(
                messages.help().contains("`cap_std::fs::Dir`"),
                "`{locale}` help lost the capability type: {}",
                messages.help()
            );
            if !locale.starts_with("en") {
                assert_ne!(
                    messages.primary(),
                    english.primary(),
                    "`{locale}` should not render the English source text"
                );
            }
        }
    }

    /// Check that the production path renders exactly what the lint's
    /// lookup resolver does, so it cannot drift to other arguments or keys.
    #[test]
    fn production_path_matches_the_lookup_resolver() {
        for locale in available_locales() {
            let localizer = Localizer::new(Some(locale));
            let resolved = localized_messages(&localizer, OPERATION)
                .unwrap_or_else(|error| panic!("`{locale}` should resolve: {error}"));
            let production = production_messages(locale);
            assert_eq!(
                production.primary(),
                normalize_fluent_output(resolved.primary().to_owned()),
                "`{locale}` production and resolver primary messages differ"
            );
            assert_eq!(
                production.help(),
                normalize_fluent_output(resolved.help().to_owned()),
                "`{locale}` production and resolver help messages differ"
            );
        }
    }

    /// Check that directional isolation marks are removed from messages.
    #[test]
    fn removes_isolation_marks() {
        let raw = "\u{2068}std::fs::read\u{2069}";
        assert_eq!(sanitize_message(raw), "std::fs::read");
    }

    /// Check that sanitizing plain text leaves it unchanged.
    #[test]
    fn preserves_clean_text() {
        let raw = "std::fs::File::open";
        assert_eq!(sanitize_message(raw), raw);
    }
}
