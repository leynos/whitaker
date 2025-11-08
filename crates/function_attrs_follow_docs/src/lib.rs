//! Lint crate enforcing doc comment placement for functions and methods.
//!
//! The lint ensures that doc comments appear before other outer attributes on
//! free functions, inherent methods, and trait methods. Keeping doc comments at
//! the front mirrors idiomatic Rust style and prevents them from being obscured
//! by implementation details such as `#[inline]` or `#[allow]` attributes.
#![feature(rustc_private)]

use common::i18n::{
    Arguments, BundleLookup, DiagnosticMessageSet, FluentValue, Localizer, MessageKey,
    MessageResolution, get_localizer_for_lint, safe_resolve_message_set,
};
#[cfg(test)]
use common::i18n::{I18nError, resolve_message_set};
use rustc_hir as hir;
use rustc_hir::Attribute;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use std::borrow::Cow;
use whitaker::SharedConfig;

/// Lint pass that validates the ordering of doc comments on functions and methods.
pub struct FunctionAttrsFollowDocs {
    localizer: Localizer,
}

impl Default for FunctionAttrsFollowDocs {
    fn default() -> Self {
        Self {
            localizer: Localizer::new(None),
        }
    }
}

dylint_linting::impl_late_lint! {
    pub FUNCTION_ATTRS_FOLLOW_DOCS,
    Warn,
    "doc comments on functions must precede other outer attributes",
    FunctionAttrsFollowDocs::default()
}

impl<'tcx> LateLintPass<'tcx> for FunctionAttrsFollowDocs {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        let shared_config = SharedConfig::load();
        self.localizer =
            get_localizer_for_lint("function_attrs_follow_docs", shared_config.locale());
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Fn { .. } = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::Function, &self.localizer);
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        if let hir::ImplItemKind::Fn(..) = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::Method, &self.localizer);
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        if let hir::TraitItemKind::Fn(..) = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::TraitMethod, &self.localizer);
        }
    }
}

#[derive(Clone, Copy)]
enum FunctionKind {
    Function,
    Method,
    TraitMethod,
}

impl Default for FunctionKind {
    fn default() -> Self {
        Self::Function
    }
}

impl FunctionKind {
    const fn subject(self) -> &'static str {
        match self {
            Self::Function => "functions",
            Self::Method => "methods",
            Self::TraitMethod => "trait methods",
        }
    }
}

struct AttrInfo {
    span: Span,
    is_doc: bool,
    is_outer: bool,
}

impl AttrInfo {
    fn from_hir(cx: &LateContext<'_>, attr: &Attribute) -> Self {
        let span = attr.span();
        let is_doc = attr.doc_str().is_some();
        let is_outer = cx
            .sess()
            .source_map()
            .span_to_snippet(span)
            .map(|snippet| !snippet.trim_start().starts_with("#!"))
            .unwrap_or(true);

        Self {
            span,
            is_doc,
            is_outer,
        }
    }
}

impl OrderedAttribute for AttrInfo {
    fn is_outer(&self) -> bool {
        self.is_outer
    }

    fn is_doc(&self) -> bool {
        self.is_doc
    }

    fn span(&self) -> Span {
        self.span
    }
}

fn check_function_attributes(
    cx: &LateContext<'_>,
    attrs: &[Attribute],
    kind: FunctionKind,
    localizer: &Localizer,
) {
    let infos: Vec<AttrInfo> = attrs
        .iter()
        .map(|attr| AttrInfo::from_hir(cx, attr))
        .collect();

    let Some((doc_index, offending_index)) = detect_misordered_doc(infos.as_slice()) else {
        return;
    };

    let doc = &infos[doc_index];
    let offending = &infos[offending_index];
    let context = DiagnosticContext {
        doc_span: doc.span(),
        offending_span: offending.span(),
        kind,
    };
    emit_diagnostic(cx, context, localizer);
}

#[derive(Copy, Clone)]
struct DiagnosticContext {
    doc_span: Span,
    offending_span: Span,
    kind: FunctionKind,
}

fn emit_diagnostic(cx: &LateContext<'_>, context: DiagnosticContext, localizer: &Localizer) {
    let attribute = attribute_label(cx, context.offending_span, localizer);
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("subject"),
        FluentValue::from(context.kind.subject()),
    );
    args.insert(
        Cow::Borrowed("attribute"),
        FluentValue::from(attribute.clone()),
    );

    let resolution = MessageResolution {
        lint_name: "function_attrs_follow_docs",
        key: MESSAGE_KEY,
        args: &args,
    };
    let messages = safe_resolve_message_set(
        localizer,
        resolution,
        |message| {
            cx.tcx
                .sess
                .dcx()
                .span_delayed_bug(context.doc_span, message);
        },
        {
            let kind = context.kind;
            move || fallback_messages(kind, attribute.as_str())
        },
    );

    cx.span_lint(FUNCTION_ATTRS_FOLLOW_DOCS, context.doc_span, |lint| {
        let primary = messages.primary();
        let note = messages.note();
        let help = messages.help();

        lint.primary_message(primary);
        lint.span_note(context.offending_span, note);
        lint.help(help);
    });
}

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("function_attrs_follow_docs");

type FunctionAttrsMessages = DiagnosticMessageSet;

#[cfg(test)]
fn localised_messages(
    lookup: &impl BundleLookup,
    kind: FunctionKind,
    attribute: &str,
) -> Result<FunctionAttrsMessages, I18nError> {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(Cow::Borrowed("subject"), FluentValue::from(kind.subject()));
    args.insert(
        Cow::Borrowed("attribute"),
        FluentValue::from(attribute.to_string()),
    );

    resolve_message_set(lookup, MESSAGE_KEY, &args)
}

fn fallback_messages(kind: FunctionKind, attribute: &str) -> FunctionAttrsMessages {
    let primary = format!(
        "Doc comments on {} must precede other outer attributes.",
        kind.subject()
    );
    let note = format!("The outer attribute {attribute} appears before the doc comment.",);
    let help = format!("Move the doc comment so it appears before {attribute} on the item.",);

    FunctionAttrsMessages::new(primary, note, help)
}

fn attribute_label(cx: &LateContext<'_>, span: Span, localizer: &Localizer) -> String {
    match cx.sess().source_map().span_to_snippet(span) {
        Ok(snippet) => snippet.trim().to_string(),
        Err(_) => attribute_fallback(localizer),
    }
}

fn attribute_fallback(lookup: &impl BundleLookup) -> String {
    let args: Arguments<'static> = Arguments::default();

    lookup
        .message(MessageKey::new("common-attribute-fallback"), &args)
        .unwrap_or_else(|_| "the preceding attribute".to_string())
}

fn detect_misordered_doc<A>(attrs: &[A]) -> Option<(usize, usize)>
where
    A: OrderedAttribute,
{
    let mut first_non_doc_outer = None;

    for (index, attribute) in attrs.iter().enumerate() {
        if !attribute.is_outer() {
            continue;
        }

        match (attribute.is_doc(), first_non_doc_outer) {
            (true, Some(non_doc_index)) => return Some((index, non_doc_index)),
            (false, None) => first_non_doc_outer = Some(index),
            _ => {}
        }
    }

    None
}

trait OrderedAttribute {
    fn is_outer(&self) -> bool;
    fn is_doc(&self) -> bool;
    fn span(&self) -> Span;
}

#[cfg(test)]
#[path = "tests/localisation.rs"]
mod localisation;

#[cfg(test)]
#[path = "tests/order_detection.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/ui.rs"]
mod ui;
