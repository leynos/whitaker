//! Localised diagnostics for the `no_std_fs_operations` lint.

use crate::NO_STD_FS_OPERATIONS;
use crate::usage::StdFsUsage;
use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    safe_resolve_message_set,
};
#[cfg(test)]
use common::i18n::{BundleLookup, I18nError, resolve_message_set};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;
use std::borrow::Cow;

/// Emit a diagnostic for a detected `std::fs` usage.
pub(crate) fn emit_diagnostic(
    cx: &LateContext<'_>,
    span: Span,
    usage: StdFsUsage,
    localizer: &Localizer,
) {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("operation"),
        FluentValue::from(usage.operation().to_string()),
    );

    let fallback_operation = usage.operation().to_string();
    let resolution = MessageResolution {
        lint_name: "no_std_fs_operations",
        key: MESSAGE_KEY,
        args: &args,
    };

    let messages = safe_resolve_message_set(
        localizer,
        resolution,
        |message| {
            cx.tcx.sess.dcx().span_delayed_bug(span, message);
        },
        move || fallback_messages(&fallback_operation),
    );

    let primary = messages.primary().to_string();
    let note = messages.note().to_string();
    let help = messages.help().to_string();

    cx.span_lint(NO_STD_FS_OPERATIONS, span, move |lint| {
        lint.primary_message(primary.clone());
        lint.note(note.clone());
        lint.help(help.clone());
    });
}

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("no_std_fs_operations");

pub(crate) type StdFsMessages = DiagnosticMessageSet;

fn fallback_messages(operation: &str) -> StdFsMessages {
    let primary = format!(
        "Avoid using std::fs operation `{operation}`; require capability-bearing handles instead."
    );
    let note = String::from(
        "std::fs reads the ambient working directory, so it bypasses the capability model enforced \
         by cap-std and camino.",
    );
    let help = String::from(
        "Pass `cap_std::fs::Dir` handles and camino::Utf8Path/Utf8PathBuf arguments down to the \
         call so only explicit capabilities touch the filesystem.",
    );
    DiagnosticMessageSet::new(primary, note, help)
}

#[cfg(test)]
pub(crate) fn localised_messages(
    lookup: &impl BundleLookup,
    operation: &str,
) -> Result<StdFsMessages, I18nError> {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("operation"),
        FluentValue::from(operation.to_string()),
    );
    resolve_message_set(lookup, MESSAGE_KEY, &args)
}
