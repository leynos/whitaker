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
        let payload = ConfigPayload::from(issue);
        emit_config_warning(cx, &ConfigWarning::new(&payload), localizer);
    }
}

/// The pre-rendered text a configuration warning interpolates.
///
/// Rendering happens when the payload is built so the message builders never
/// have to decide between a key list and a parse error.
enum ConfigPayload {
    /// Top-level keys this suite ignored; the known keys still apply.
    UnknownKeys(String),
    /// The parse error that discarded the whole section.
    DiscardedSection(String),
}

impl From<&ConfigIssue> for ConfigPayload {
    /// Render one configuration issue as a bounded diagnostic payload.
    fn from(issue: &ConfigIssue) -> Self {
        match issue {
            ConfigIssue::UnknownKeys(keys) => Self::UnknownKeys(format_key_list(keys)),
            ConfigIssue::SectionDiscarded(error) => Self::DiscardedSection(bounded_entry(error)),
        }
    }
}

impl ConfigPayload {
    /// Return the Fluent argument name the resources interpolate.
    const fn argument_name(&self) -> &'static str {
        match self {
            Self::UnknownKeys(_) => "keys",
            Self::DiscardedSection(_) => "error",
        }
    }

    /// Return the Fluent message key that describes this payload.
    const fn message_key(&self) -> MessageKey<'static> {
        match self {
            Self::UnknownKeys(_) => UNKNOWN_KEYS_MESSAGE_KEY,
            Self::DiscardedSection(_) => DISCARDED_SECTION_MESSAGE_KEY,
        }
    }

    /// Return the rendered text to interpolate.
    fn text(&self) -> &str {
        match self {
            Self::UnknownKeys(text) | Self::DiscardedSection(text) => text,
        }
    }
}

/// A configuration warning and the suite provenance it reports.
///
/// Keeping the payload and the revision together lets the message builders
/// take one value instead of separate, easily transposed strings.
struct ConfigWarning<'a> {
    payload: &'a ConfigPayload,
    revision: &'static str,
}

impl<'a> ConfigWarning<'a> {
    /// Capture a payload and report the compiled suite's revision.
    fn new(payload: &'a ConfigPayload) -> Self {
        Self {
            payload,
            revision: suite_revision(),
        }
    }

    /// Capture a payload with an explicit revision, for tests that pin it.
    #[cfg(test)]
    const fn with_revision(payload: &'a ConfigPayload, revision: &'static str) -> Self {
        Self { payload, revision }
    }

    /// Build the Fluent interpolation arguments the resources expect.
    fn arguments(&self) -> Arguments<'static> {
        let mut args: Arguments<'static> = Arguments::default();
        args.insert(
            Cow::Borrowed(self.payload.argument_name()),
            FluentValue::from(self.payload.text().to_owned()),
        );
        args.insert(
            Cow::Borrowed("version"),
            FluentValue::from(PACKAGE_VERSION.to_owned()),
        );
        args.insert(
            Cow::Borrowed("revision"),
            FluentValue::from(self.revision.to_owned()),
        );
        args
    }

    /// Build the English fallback used when localization is unavailable.
    fn fallback(&self) -> DiagnosticMessageSet {
        let (primary, note, help) = match self.payload {
            ConfigPayload::UnknownKeys(_) => (
                format!(
                    concat!(
                        "Ignoring unknown key(s) {keys} in [no_std_fs_operations] for ",
                        "no_std_fs_operations {version} (revision {revision})."
                    ),
                    keys = self.payload.text(),
                    revision = self.revision,
                    version = PACKAGE_VERSION,
                ),
                UNKNOWN_KEYS_NOTE,
                UNKNOWN_KEYS_HELP,
            ),
            ConfigPayload::DiscardedSection(_) => (
                format!(
                    concat!(
                        "Discarding the [no_std_fs_operations] configuration for ",
                        "no_std_fs_operations {version} (revision {revision}): {error}"
                    ),
                    error = self.payload.text(),
                    revision = self.revision,
                    version = PACKAGE_VERSION,
                ),
                DISCARDED_SECTION_NOTE,
                DISCARDED_SECTION_HELP,
            ),
        };
        DiagnosticMessageSet::new(primary, note.to_owned(), help.to_owned())
    }
}

/// Emit one configuration warning as a spanless rustc warning.
fn emit_config_warning(cx: &LateContext<'_>, warning: &ConfigWarning<'_>, localizer: &Localizer) {
    let args = warning.arguments();
    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: warning.payload.message_key(),
        args: &args,
    };
    let messages =
        safe_resolve_message_set(localizer, resolution, noop_reporter, || warning.fallback());
    emit_warning(cx, &messages);
}

/// Render the unknown-key names as one quoted, bounded list.
fn format_key_list(keys: &[String]) -> String {
    keys.iter()
        .map(|key| bounded_entry(key))
        .collect::<Vec<_>>()
        .join(", ")
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

/// English fallback note for ignored unknown keys.
const UNKNOWN_KEYS_NOTE: &str = concat!(
    "Other keys in this section still apply; the unrecognized key(s) have ",
    "no effect."
);

/// English fallback help for ignored unknown keys.
const UNKNOWN_KEYS_HELP: &str = concat!(
    "Update the Whitaker suite to a revision that understands these keys, ",
    "or remove them from dylint.toml."
);

/// English fallback note for a discarded configuration section.
const DISCARDED_SECTION_NOTE: &str = "No exclusions apply; the lint is using its default settings.";

/// English fallback help for a discarded configuration section.
const DISCARDED_SECTION_HELP: &str = concat!(
    "Fix the invalid value in dylint.toml, or update the Whitaker suite so ",
    "it understands the key."
);

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("no_std_fs_operations");

const UNKNOWN_KEYS_MESSAGE_KEY: MessageKey<'static> =
    MessageKey::new("no_std_fs_operations_unknown_keys");

const DISCARDED_SECTION_MESSAGE_KEY: MessageKey<'static> =
    MessageKey::new("no_std_fs_operations_discarded_section");

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
#[path = "diagnostics_tests.rs"]
mod tests;
