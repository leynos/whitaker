//! Localized diagnostic emission and primary-span selection.
//!
//! Messages flow through the shared `whitaker_common::i18n` resolution helpers
//! with an English fallback, so the translations added by roadmap item 2.2.14
//! only add resources and never touch this module. The localizer is loaded once
//! per process: a pre-expansion pass is instantiated again for every
//! out-of-line module file, and most of those runs never reach `check_crate`,
//! so no per-crate hook could initialize it.

use std::sync::OnceLock;

use rustc_lint::{EarlyContext, LintContext, errors::DiagDecorator};
use rustc_span::Span;
use whitaker_common::{
    Arguments,
    Localizer,
    i18n::{
        DiagnosticMessageSet,
        MessageKey,
        MessageResolution,
        get_localizer_for_lint,
        noop_reporter,
        safe_resolve_message_set,
    },
};
use whitaker_lint_core::SharedConfig;

use super::{LINT_NAME, MISSING_DOCS_IN_TEST_FUNCTIONS};

/// Fluent message key resolved for this lint's diagnostic.
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

/// The process-wide localizer, loaded on first diagnosis.
static LOCALIZER: OnceLock<Localizer> = OnceLock::new();

/// Emits the missing-documentation diagnostic at `span`.
pub(super) fn emit_undocumented(cx: &EarlyContext<'_>, span: Span, function_name: &str) {
    let arguments = Arguments::default();
    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &arguments,
    };
    let messages = safe_resolve_message_set(localizer(), resolution, noop_reporter, || {
        fallback_messages(function_name)
    });

    cx.emit_span_lint(
        MISSING_DOCS_IN_TEST_FUNCTIONS,
        span,
        DiagDecorator(move |diagnostic| {
            diagnostic.primary_message(messages.primary().to_owned());
            diagnostic.help(messages.help().to_owned());
        }),
    );
}

/// Selects the span the diagnostic is attached to.
///
/// The function's identifier is the useful place to point at, but a synthetic
/// or re-parsed item can carry a dummy identifier span. The item span is the
/// fallback: it still names the function the diagnostic is about, whereas a
/// dummy span would be dropped from the report.
pub(super) fn diagnostic_span(identifier: Span, item: Span) -> Span {
    if identifier.is_dummy() {
        item
    } else {
        identifier
    }
}

/// Returns the process-wide localizer, loading it on first use.
///
/// This is the crate's one ambient read, and it happens at the emit boundary on
/// purpose. Loading resolves the configured locale from `dylint.toml` through
/// [`SharedConfig::load`] and from `DYLINT_LOCALE` through
/// `get_localizer_for_lint`; the result is cached in [`LOCALIZER`] because a
/// lint cannot install state during startup and must not re-read configuration
/// for every diagnostic. The value is written once and immutable afterwards, so
/// the cache hides the dependency's cost rather than its meaning: the first
/// diagnostic in a process is the point at which the environment is consulted.
fn localizer() -> &'static Localizer {
    LOCALIZER.get_or_init(|| {
        let shared_config = SharedConfig::load();
        get_localizer_for_lint(LINT_NAME, shared_config.locale())
    })
}

/// Supplies the English message set used when the locale bundle is unusable.
///
/// A missing or malformed bundle must not suppress the finding; the lint is
/// still correct, only its wording degrades.
fn fallback_messages(function_name: &str) -> DiagnosticMessageSet {
    DiagnosticMessageSet::new(
        format!("test function `{function_name}` has no documentation comment"),
        String::from(
            "test functions are easier to review when their documentation states what behaviour \
             they exercise",
        ),
        String::from(
            "add a `///` comment above the function and its attributes that explains what the \
             test exercises",
        ),
    )
}
