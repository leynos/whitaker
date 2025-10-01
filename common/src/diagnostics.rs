use rustc_errors::Applicability;
use rustc_lint::{LateContext, Lint, LintContext};
use rustc_span::Span;

#[must_use]
pub fn span_lint(
    cx: &LateContext<'_>,
    lint: &'static Lint,
    span: Span,
    message: impl Into<String>,
) {
    let message = message.into();
    cx.opt_span_lint(lint, Some(span), move |diagnostic| {
        diagnostic.primary_message(message);
    });
}

#[must_use]
pub fn span_lint_and_help(
    cx: &LateContext<'_>,
    lint: &'static Lint,
    span: Span,
    message: impl Into<String>,
    help: impl Into<String>,
) {
    let message = message.into();
    let help = help.into();
    cx.opt_span_lint(lint, Some(span), move |diagnostic| {
        diagnostic.primary_message(message);
        diagnostic.help(help);
    });
}

#[must_use]
pub fn span_lint_and_sugg(
    cx: &LateContext<'_>,
    lint: &'static Lint,
    span: Span,
    message: impl Into<String>,
    suggestion: impl Into<String>,
    replacement: impl Into<String>,
    applicability: Applicability,
) {
    let message = message.into();
    let suggestion = suggestion.into();
    let replacement = replacement.into();
    cx.opt_span_lint(lint, Some(span), move |diagnostic| {
        diagnostic.primary_message(message);
        diagnostic.span_suggestion(span, suggestion, replacement, applicability);
    });
}
