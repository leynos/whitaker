use crate::NO_EXPECT_OUTSIDE_TESTS;
use crate::context::ContextSummary;
use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    safe_resolve_message_set,
};
#[cfg(test)]
use common::i18n::{BundleLookup, I18nError, resolve_message_set};
use rustc_hir as hir;
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::sym;
use std::borrow::Cow;
use std::fmt;

/// A formatted label for the receiver type (e.g., "`Result<T, E>`").
#[derive(Debug, Clone)]
pub(crate) struct ReceiverLabel(String);

impl ReceiverLabel {
    pub(crate) fn new(label: impl Into<String>) -> Self {
        Self(label.into())
    }
}

impl Default for ReceiverLabel {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl AsRef<str> for ReceiverLabel {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReceiverLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReceiverCategory {
    Option,
    Result,
    Other,
}

impl ReceiverCategory {
    fn classify_ty(cx: &LateContext<'_>, ty: ty::Ty<'_>) -> Self {
        match ty.kind() {
            ty::Adt(adt, _) => {
                let did = adt.did();
                if cx.tcx.is_diagnostic_item(sym::Option, did) {
                    Self::Option
                } else if cx.tcx.is_diagnostic_item(sym::Result, did) {
                    Self::Result
                } else {
                    Self::Other
                }
            }
            _ => Self::Other,
        }
    }

    #[cfg(test)]
    fn for_label(receiver: &ReceiverLabel) -> Self {
        let value = receiver.as_ref();
        if value.contains("Option") {
            Self::Option
        } else if value.contains("Result") {
            Self::Result
        } else {
            Self::Other
        }
    }

    fn as_key(self) -> &'static str {
        match self {
            Self::Option => "option",
            Self::Result => "result",
            Self::Other => "other",
        }
    }

    fn fallback_help(self, receiver: &ReceiverLabel) -> String {
        match self {
            Self::Option => {
                format!("Handle the `None` case for {receiver} or move the code into a test.")
            }
            Self::Result => {
                format!("Handle the `Err` variant of {receiver} or move the code into a test.")
            }
            Self::Other => {
                format!("Handle the error path for {receiver} or move the code into a test.")
            }
        }
    }
}

/// A formatted label for the call context (e.g., "function `handler`" or "the surrounding scope").
#[derive(Debug, Clone)]
pub(crate) struct ContextLabel(String);

impl ContextLabel {
    pub(crate) fn new(label: impl Into<String>) -> Self {
        Self(label.into())
    }
}

impl AsRef<str> for ContextLabel {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContextLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub(crate) struct DiagnosticContext<'a> {
    pub(crate) summary: &'a ContextSummary,
    pub(crate) localizer: &'a Localizer,
}

impl<'a> DiagnosticContext<'a> {
    pub(crate) fn new(summary: &'a ContextSummary, localizer: &'a Localizer) -> Self {
        Self { summary, localizer }
    }
}

pub(crate) fn emit_diagnostic(
    cx: &LateContext<'_>,
    expr: &hir::Expr<'_>,
    receiver: &hir::Expr<'_>,
    context: &DiagnosticContext<'_>,
) {
    let receiver_ty = cx.typeck_results().expr_ty(receiver).peel_refs();
    let receiver_label = ReceiverLabel::new(format!("`{}`", receiver_ty));
    let call_context = context_label(context.summary);

    let category = ReceiverCategory::classify_ty(cx, receiver_ty);

    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("receiver"),
        FluentValue::from(receiver_label.as_ref().to_string()),
    );
    args.insert(
        Cow::Borrowed("context"),
        FluentValue::from(call_context.as_ref().to_string()),
    );
    args.insert(
        Cow::Borrowed("handling"),
        FluentValue::from(category.as_key().to_string()),
    );

    let fallback_receiver = receiver_label.clone();
    let fallback_context = call_context.clone();
    let resolution = MessageResolution {
        lint_name: "no_expect_outside_tests",
        key: MESSAGE_KEY,
        args: &args,
    };
    let messages = safe_resolve_message_set(
        context.localizer,
        resolution,
        |message| {
            cx.tcx.sess.dcx().span_delayed_bug(expr.span, message);
        },
        move || fallback_messages(&fallback_receiver, &fallback_context, category),
    );

    let primary = messages.primary().to_string();
    let note = messages.note().to_string();
    let help = messages.help().to_string();

    cx.span_lint(NO_EXPECT_OUTSIDE_TESTS, expr.span, move |lint| {
        lint.primary_message(primary.clone());
        lint.note(note.clone());
        lint.help(help.clone());
    });
}

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("no_expect_outside_tests");

type NoExpectMessages = DiagnosticMessageSet;

#[cfg(test)]
fn localised_messages(
    lookup: &impl BundleLookup,
    receiver: &ReceiverLabel,
    context: &ContextLabel,
    category: ReceiverCategory,
) -> Result<NoExpectMessages, I18nError> {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("receiver"),
        FluentValue::from(receiver.as_ref().to_string()),
    );
    args.insert(
        Cow::Borrowed("context"),
        FluentValue::from(context.as_ref().to_string()),
    );
    args.insert(
        Cow::Borrowed("handling"),
        FluentValue::from(category.as_key().to_string()),
    );

    resolve_message_set(lookup, MESSAGE_KEY, &args)
}

fn fallback_messages(
    receiver: &ReceiverLabel,
    context: &ContextLabel,
    category: ReceiverCategory,
) -> NoExpectMessages {
    let primary = format!("Avoid calling expect on {receiver} outside test-only code.");
    let note = format!("The call originates within {context} which is not recognised as a test.",);
    let help = category.fallback_help(receiver);

    NoExpectMessages::new(primary, note, help)
}

fn context_label(summary: &ContextSummary) -> ContextLabel {
    let label = summary
        .function_name
        .as_ref()
        .map(|name| format!("function `{name}`"))
        .unwrap_or_else(|| "the surrounding scope".to_string());

    ContextLabel::new(label)
}

#[cfg(test)]
#[path = "tests/localisation.rs"]
mod localisation;

#[cfg(test)]
#[path = "tests/receiver_type_edge_cases.rs"]
mod receiver_type_edge_cases;
