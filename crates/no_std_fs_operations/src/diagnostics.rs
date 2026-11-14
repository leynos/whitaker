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

use crate::{
    NO_STD_FS_OPERATIONS,
    config::{ConfigIssue, LINT_NAME, PACKAGE_VERSION, suite_revision},
    exclusion::bounded_entry,
    usage::StdFsUsage,
};

/// Emit a diagnostic for a detected `std::fs` usage.
pub(crate) fn emit_diagnostic(
    cx: &LateContext<'_>,
    span: Span,
    usage: &StdFsUsage,
    localizer: &Localizer,
) {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("operation"),
        FluentValue::from(usage.operation().to_owned()),
    );

    let fallback_operation = usage.operation().to_owned();
    let resolution = MessageResolution {
        lint_name: "no_std_fs_operations",
        key: MESSAGE_KEY,
        args: &args,
    };

    let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, move || {
        fallback_messages(&fallback_operation)
    });

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

/// Report configuration problems through Cargo-visible rustc warnings.
///
/// A plain rustc warning is used instead of a lint diagnostic so the notice is
/// still emitted when the crate is excluded from the lint. Each issue is
/// reported once per crate compilation because the driver calls this from
/// `check_crate`.
pub(crate) fn emit_config_issues(
    cx: &LateContext<'_>,
    issues: &[ConfigIssue],
    localizer: &Localizer,
) {
    for issue in issues {
        match issue {
            ConfigIssue::UnknownKeys(keys) => emit_unknown_keys(cx, keys, localizer),
            ConfigIssue::SectionDiscarded(error) => emit_discarded_section(cx, error, localizer),
        }
    }
}

/// Report that some top-level keys were ignored while the known keys remain
/// in force.
fn emit_unknown_keys(cx: &LateContext<'_>, keys: &[String], localizer: &Localizer) {
    let key_list = format_key_list(keys);
    let revision = suite_revision();
    let mut args: Arguments<'static> = Arguments::default();
    insert_argument(&mut args, "keys", key_list.clone());
    insert_argument(&mut args, "version", PACKAGE_VERSION.to_owned());
    insert_argument(&mut args, "revision", revision.to_owned());
    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: UNKNOWN_KEYS_MESSAGE_KEY,
        args: &args,
    };
    let fallback_keys = key_list;
    let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, move || {
        fallback_unknown_keys(&fallback_keys, revision)
    });
    emit_warning(cx, &messages);
}

/// Report that the whole section was discarded because a known value did not
/// parse.
fn emit_discarded_section(cx: &LateContext<'_>, error: &str, localizer: &Localizer) {
    let error_text = bounded_entry(error);
    let revision = suite_revision();
    let mut args: Arguments<'static> = Arguments::default();
    insert_argument(&mut args, "error", error_text.clone());
    insert_argument(&mut args, "version", PACKAGE_VERSION.to_owned());
    insert_argument(&mut args, "revision", revision.to_owned());
    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: DISCARDED_SECTION_MESSAGE_KEY,
        args: &args,
    };
    let fallback_error = error_text;
    let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, move || {
        fallback_discarded_section(&fallback_error, revision)
    });
    emit_warning(cx, &messages);
}

/// Render the unknown-key names as one quoted, bounded list.
fn format_key_list(keys: &[String]) -> String {
    keys.iter()
        .map(|key| bounded_entry(key))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Add one Fluent interpolation argument.
fn insert_argument(args: &mut Arguments<'static>, name: &'static str, value: String) {
    args.insert(Cow::Borrowed(name), FluentValue::from(value));
}

/// Emit a spanless warning from the resolved message set.
fn emit_warning(cx: &LateContext<'_>, messages: &DiagnosticMessageSet) {
    let mut diagnostic = cx
        .tcx
        .sess
        .dcx()
        .struct_warn(sanitize_message(messages.primary()));
    diagnostic.note(sanitize_message(messages.note()));
    diagnostic.help(sanitize_message(messages.help()));
    diagnostic.emit();
}

/// Build the English fallback for ignored unknown keys.
fn fallback_unknown_keys(keys: &str, revision: &str) -> DiagnosticMessageSet {
    let primary = format!(
        concat!(
            "Ignoring unknown key(s) {keys} in [no_std_fs_operations] for ",
            "no_std_fs_operations {version} (revision {revision})."
        ),
        keys = keys,
        revision = revision,
        version = PACKAGE_VERSION,
    );
    let note = concat!(
        "Other keys in this section still apply; the unrecognized key(s) have ",
        "no effect."
    )
    .to_owned();
    let help = concat!(
        "Update the Whitaker suite to a revision that understands these keys, ",
        "or remove them from dylint.toml."
    )
    .to_owned();
    DiagnosticMessageSet::new(primary, note, help)
}

/// Build the English fallback for a discarded configuration section.
fn fallback_discarded_section(error: &str, revision: &str) -> DiagnosticMessageSet {
    let primary = format!(
        concat!(
            "Discarding the [no_std_fs_operations] configuration for ",
            "no_std_fs_operations {version} (revision {revision}): {error}"
        ),
        error = error,
        revision = revision,
        version = PACKAGE_VERSION,
    );
    let note = "No exclusions apply; the lint is using its default settings.".to_owned();
    let help = concat!(
        "Fix the invalid value in dylint.toml, or update the Whitaker suite so ",
        "it understands the key."
    )
    .to_owned();
    DiagnosticMessageSet::new(primary, note, help)
}

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("no_std_fs_operations");

const UNKNOWN_KEYS_MESSAGE_KEY: MessageKey<'static> =
    MessageKey::new("no_std_fs_operations_unknown_keys");

const DISCARDED_SECTION_MESSAGE_KEY: MessageKey<'static> =
    MessageKey::new("no_std_fs_operations_discarded_section");

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
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("operation"),
        FluentValue::from(operation.to_owned()),
    );
    resolve_message_set(lookup, MESSAGE_KEY, &args)
}

#[cfg(test)]
mod tests {
    //! Tests for diagnostic message sanitization and configuration message
    //! resolution in every supported locale.

    use std::borrow::Cow;

    use rstest::rstest;
    use whitaker_common::i18n::{
        Arguments,
        FluentValue,
        Localizer,
        MessageKey,
        resolve_message_set,
    };

    use super::{
        DISCARDED_SECTION_MESSAGE_KEY,
        PACKAGE_VERSION,
        UNKNOWN_KEYS_MESSAGE_KEY,
        sanitize_message,
    };

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

    #[rstest]
    #[case::english("en-GB")]
    #[case::welsh("cy")]
    #[case::gaelic("gd")]
    fn unknown_key_messages_resolve_for_each_locale(#[case] locale: &str) {
        let messages = resolve_config_messages(
            locale,
            UNKNOWN_KEYS_MESSAGE_KEY,
            &[("keys", "\"future_option\""), ("error", "unused")],
        );

        assert!(messages.primary().contains("[no_std_fs_operations]"));
        assert!(messages.primary().contains("future_option"));
        assert!(messages.primary().contains(PACKAGE_VERSION));
        assert!(messages.primary().contains("abc1234"));
        assert!(!messages.note().is_empty());
        assert!(!messages.help().is_empty());
    }

    #[rstest]
    #[case::english("en-GB")]
    #[case::welsh("cy")]
    #[case::gaelic("gd")]
    fn discarded_section_messages_resolve_for_each_locale(#[case] locale: &str) {
        let messages = resolve_config_messages(
            locale,
            DISCARDED_SECTION_MESSAGE_KEY,
            &[("keys", "unused"), ("error", "\"bad value\"")],
        );

        assert!(messages.primary().contains("[no_std_fs_operations]"));
        assert!(messages.primary().contains("bad value"));
        assert!(messages.primary().contains(PACKAGE_VERSION));
        assert!(messages.primary().contains("abc1234"));
        assert!(!messages.note().is_empty());
        assert!(!messages.help().is_empty());
    }

    /// Resolve one configuration message with every interpolation argument the
    /// Fluent resources expect.
    fn resolve_config_messages(
        locale: &str,
        key: MessageKey<'static>,
        values: &[(&'static str, &str)],
    ) -> whitaker_common::i18n::DiagnosticMessageSet {
        let localizer = Localizer::new(Some(locale));
        let mut args: Arguments<'static> = Arguments::default();
        for (name, value) in values {
            args.insert(Cow::Borrowed(*name), FluentValue::from((*value).to_owned()));
        }
        args.insert(
            Cow::Borrowed("version"),
            FluentValue::from(PACKAGE_VERSION.to_owned()),
        );
        args.insert(
            Cow::Borrowed("revision"),
            FluentValue::from("abc1234".to_owned()),
        );
        resolve_message_set(&localizer, key, &args)
            .expect("configuration messages should resolve for every supported locale")
    }
}
