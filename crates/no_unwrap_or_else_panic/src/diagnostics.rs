//! Diagnostic emission for the lint, including localisation fallbacks.

use crate::{LINT_NAME, NO_UNWRAP_OR_ELSE_PANIC};
use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    noop_reporter, safe_resolve_message_set,
};
use rustc_hir as hir;
use rustc_lint::{LateContext, LintContext};
use std::borrow::Cow;

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

/// Emit the lint diagnostic using localised messages.
///
/// # Examples
///
/// ```rust,ignore
/// // Called from a lint driver once `call` and `receiver` are known:
/// // let localizer = resolve_localizer(...);
/// // emit_diagnostic(cx, call, receiver, &localizer);
/// ```
pub(crate) fn emit_diagnostic(
    cx: &LateContext<'_>,
    expr: &hir::Expr<'_>,
    receiver: &hir::Expr<'_>,
    localizer: &Localizer,
) {
    let receiver_label = format!("`{}`", cx.typeck_results().expr_ty(receiver).peel_refs());

    let mut args: Arguments<'_> = Arguments::default();
    args.insert(
        Cow::Borrowed("receiver"),
        FluentValue::from(receiver_label.as_str()),
    );

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };

    let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, || {
        fallback_messages(&receiver_label)
    });

    cx.span_lint(NO_UNWRAP_OR_ELSE_PANIC, expr.span, |lint| {
        lint.primary_message(messages.primary().to_string());
        lint.span_note(receiver.span, messages.note().to_string());
        lint.help(messages.help().to_string());
    });
}

fn fallback_messages(receiver: &str) -> DiagnosticMessageSet {
    let primary = format!("Replace unwrap_or_else with a non-panicking fallback on {receiver}.");
    let note = String::from("The closure supplied to unwrap_or_else triggers a panic.");
    let help =
        String::from("Propagate the error or use expect with a descriptive message instead.");

    DiagnosticMessageSet::new(primary, note, help)
}
