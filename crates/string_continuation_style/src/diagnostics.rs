//! Emits localized diagnostics for rewrites that pass source-span and value
//! checks, with applicability selected from the lint configuration.

use rustc_lint::{Applicability, EarlyContext, LintContext, errors::DiagDecorator};
use whitaker::SharedConfig;
use whitaker_common::i18n::{
    AttrKey,
    DiagnosticMessageSet,
    LabelResolution,
    Localizer,
    MessageKey,
    MessageResolution,
    get_localizer_for_lint,
    noop_reporter,
    safe_resolve_label,
    safe_resolve_message_set,
};

use crate::{
    config::{ApplicabilityMode, LINT_NAME},
    driver::{LiteralRewrite, STRING_CONTINUATION_STYLE},
};

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

/// Loads the locale selected by Dylint configuration and environment.
pub(crate) fn load_localizer() -> Localizer {
    let shared_config = SharedConfig::load();
    get_localizer_for_lint(LINT_NAME, shared_config.locale())
}

/// Emits one diagnostic for all joins in a validated literal.
pub(crate) fn emit_rewrite(
    cx: &EarlyContext<'_>,
    localizer: &Localizer,
    applicability_mode: ApplicabilityMode,
    rewrite: LiteralRewrite,
) {
    let arguments = whitaker_common::Arguments::default();
    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &arguments,
    };
    let messages =
        safe_resolve_message_set(localizer, resolution, noop_reporter, fallback_messages);
    let labels = rewrite
        .join_spans
        .into_iter()
        .map(|span| (span, resolve_join_label(localizer, resolution)))
        .collect::<Vec<_>>();
    let suggestion = resolve_suggestion(localizer, resolution);
    let applicability = applicability(applicability_mode);

    cx.emit_span_lint(
        STRING_CONTINUATION_STYLE,
        rewrite.literal_span,
        DiagDecorator(move |diagnostic| {
            diagnostic.primary_message(messages.primary().to_owned());
            diagnostic.span_note(rewrite.literal_span, messages.note().to_owned());
            for (span, label) in labels {
                diagnostic.span_label(span, label);
            }
            diagnostic.help(messages.help().to_owned());
            diagnostic.span_suggestion(
                rewrite.literal_span,
                suggestion,
                rewrite.replacement,
                applicability,
            );
        }),
    );
}

fn applicability(mode: ApplicabilityMode) -> Applicability {
    match mode {
        ApplicabilityMode::Manual => Applicability::MaybeIncorrect,
        ApplicabilityMode::Machine => Applicability::MachineApplicable,
    }
}

fn resolve_join_label(localizer: &Localizer, message: MessageResolution<'_>) -> String {
    safe_resolve_label(
        localizer,
        LabelResolution {
            message,
            attribute: AttrKey::new("label"),
        },
        noop_reporter,
        || String::from("source-line join"),
    )
}

fn resolve_suggestion(localizer: &Localizer, message: MessageResolution<'_>) -> String {
    safe_resolve_label(
        localizer,
        LabelResolution {
            message,
            attribute: AttrKey::new("suggestion"),
        },
        noop_reporter,
        || String::from("replace with a `concat!()` expression"),
    )
}

fn fallback_messages() -> DiagnosticMessageSet {
    DiagnosticMessageSet::new(
        String::from("join these string fragments with `concat!()`"),
        String::from(
            "the continuation escape removes the physical newline and following indentation, \
             hiding the fragment boundary inside one literal",
        ),
        String::from("split the cooked string into `concat!()` arguments at each source-line join"),
    )
}

#[cfg(test)]
#[path = "../tests/support/diagnostics_tests.rs"]
mod tests;
