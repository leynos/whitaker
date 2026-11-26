//! Lint crate enforcing doc comment placement for functions and methods.
//!
//! The lint ensures that doc comments appear before other outer attributes on
//! free functions, inherent methods, and trait methods. Keeping doc comments at
//! the front mirrors idiomatic Rust style and prevents them from being obscured
//! by implementation details such as `#[inline]` or `#[allow]` attributes.
#[path = "driver_attr_info.rs"]
mod attr_info;

use std::borrow::Cow;

use rustc_hir as hir;
use rustc_lint::{DiagDecorator, LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use whitaker_common::i18n::{
    Arguments,
    BundleLookup,
    DiagnosticMessageSet,
    FluentValue,
    Localizer,
    MessageKey,
    MessageResolution,
    get_localizer_for_lint,
    noop_reporter,
    safe_resolve_message_set,
};
#[cfg(test)]
use whitaker_common::i18n::{I18nError, resolve_message_set};
use whitaker_lint_core::{SharedConfig, recover_user_editable_hir_span};

use self::attr_info::AttrInfo;
#[cfg(test)]
use self::attr_info::parsed_attribute_span;

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

mod declaration {
    //! Dylint lint declaration and registration glue.
    //!
    //! `impl_late_lint!` expands to the Dylint ABI entry point and the
    //! `impl_lint_pass!` accessor, neither of which has a source location that
    //! could carry documentation. Isolating the invocation keeps the expectation
    //! scoped to exactly those generated items.

    #![expect(
        missing_docs,
        reason = "dylint_linting macro expansion emits items with no documentable source location"
    )]

    use super::FunctionAttrsFollowDocs;

    dylint_linting::impl_late_lint! {
        /// Warns when outer attributes on a function precede its doc comments.
        pub FUNCTION_ATTRS_FOLLOW_DOCS,
        Warn,
        "doc comments on functions must precede other outer attributes",
        FunctionAttrsFollowDocs::default()
    }
}

pub use declaration::FUNCTION_ATTRS_FOLLOW_DOCS;

impl<'tcx> LateLintPass<'tcx> for FunctionAttrsFollowDocs {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        let shared_config = SharedConfig::load();
        self.localizer =
            get_localizer_for_lint("function_attrs_follow_docs", shared_config.locale());
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Fn { .. } = item.kind {
            self.check_item_attributes(
                cx,
                &ItemInfo::new(item.hir_id(), item.span, FunctionKind::Function),
            );
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        if let hir::ImplItemKind::Fn(..) = item.kind {
            self.check_item_attributes(
                cx,
                &ItemInfo::new(item.hir_id(), item.span, FunctionKind::Method),
            );
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        if let hir::TraitItemKind::Fn(..) = item.kind {
            self.check_item_attributes(
                cx,
                &ItemInfo::new(item.hir_id(), item.span, FunctionKind::TraitMethod),
            );
        }
    }
}

/// Information about a function or method item to be checked.
struct ItemInfo {
    hir_id: hir::HirId,
    span: Span,
    kind: FunctionKind,
}

impl ItemInfo {
    /// Capture the HIR identity, source span, and function kind.
    const fn new(hir_id: hir::HirId, span: Span, kind: FunctionKind) -> Self {
        Self { hir_id, span, kind }
    }
}

impl<'tcx> FunctionAttrsFollowDocs {
    /// Check the attributes attached to one function-like item.
    fn check_item_attributes(&self, cx: &LateContext<'tcx>, item: &ItemInfo) {
        let attrs = cx.tcx.hir_attrs(item.hir_id);
        check_function_attributes(&FunctionAttributeCheck {
            cx,
            attrs,
            item_span: item.span,
            kind: item.kind,
            localizer: &self.localizer,
        });
    }
}

#[derive(Clone, Copy, Default)]
enum FunctionKind {
    #[default]
    Function,
    Method,
    TraitMethod,
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

/// Context for checking function attributes.
struct FunctionAttributeCheck<'tcx, 'a> {
    cx: &'a LateContext<'tcx>,
    attrs: &'a [hir::Attribute],
    item_span: Span,
    kind: FunctionKind,
    localizer: &'a Localizer,
}

/// Report function attributes that appear after the item's documentation.
fn check_function_attributes(check: &FunctionAttributeCheck<'_, '_>) {
    let item_user_editable_span = recover_user_editable_hir_span(check.item_span);
    let mut infos: Vec<AttrInfo> = check
        .attrs
        .iter()
        .filter_map(AttrInfo::try_from_hir)
        .collect();
    infos.retain(|info| {
        attribute_within_item(
            info.user_editable_span(),
            item_user_editable_span,
            check.item_span,
        )
    });
    // Attribute macros can reorder attributes in HIR; rely on source order instead.
    infos.sort_by_key(AttrInfo::source_order_key);

    let Some((doc_index, offending_index)) = detect_misordered_doc(infos.as_slice()) else {
        return;
    };

    let (Some(doc), Some(offending)) = (infos.get(doc_index), infos.get(offending_index)) else {
        return;
    };
    let diagnostic_context = DiagnosticContext {
        doc_span: doc.span(),
        offending_span: offending.span(),
        kind: check.kind,
    };
    emit_diagnostic(check.cx, diagnostic_context, check.localizer);
}

/// Returns true when the attribute span falls within the item span.
///
/// Dummy item spans are treated as in-bounds. Attributes with no recoverable
/// user-editable span are discarded so the lint never compares macro-only glue.
/// When item-span recovery fails, the raw item span remains the containment
/// fallback for user-authored items.
fn attribute_within_item(
    attribute_span: Option<Span>,
    item_span: Option<Span>,
    raw_item_span: Span,
) -> bool {
    let Some(editable_attribute_span) = attribute_span else {
        return false;
    };

    if raw_item_span.is_dummy() {
        return true;
    }

    let bounding_item_span = item_span.unwrap_or(raw_item_span);

    // Modern nightlies exclude attributes from the item span, so outer
    // attributes sit immediately before it. Accept spans contained in the
    // item (older behaviour and inner attributes) or preceding it (outer
    // attributes on current nightlies).
    let contained = editable_attribute_span.lo() >= bounding_item_span.lo()
        && editable_attribute_span.hi() <= bounding_item_span.hi();
    let precedes = editable_attribute_span.hi() <= bounding_item_span.lo();
    contained || precedes
}

#[derive(Copy, Clone)]
struct DiagnosticContext {
    doc_span: Span,
    offending_span: Span,
    kind: FunctionKind,
}

/// Emit a localized diagnostic for an attribute that follows documentation.
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
    let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, {
        let kind = context.kind;
        move || fallback_messages(kind, attribute.as_str())
    });
    let primary = messages.primary().to_owned();
    let note = messages.note().to_owned();
    let help = messages.help().to_owned();

    cx.emit_span_lint(
        FUNCTION_ATTRS_FOLLOW_DOCS,
        context.doc_span,
        DiagDecorator(move |lint| {
            lint.primary_message(primary);
            lint.span_note(context.offending_span, note);
            lint.help(help);
        }),
    );
}

const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("function_attrs_follow_docs");

type FunctionAttrsMessages = DiagnosticMessageSet;

/// Build localized diagnostics for an attribute kind using the supplied lookup.
#[cfg(test)]
fn localized_messages(
    lookup: &impl BundleLookup,
    kind: FunctionKind,
    attribute: &str,
) -> Result<FunctionAttrsMessages, I18nError> {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(Cow::Borrowed("subject"), FluentValue::from(kind.subject()));
    args.insert(
        Cow::Borrowed("attribute"),
        FluentValue::from(attribute.to_owned()),
    );

    resolve_message_set(lookup, MESSAGE_KEY, &args)
}

/// Build diagnostic text when localized messages cannot be loaded.
fn fallback_messages(kind: FunctionKind, attribute: &str) -> FunctionAttrsMessages {
    let primary = format!(
        "Doc comments on {} must precede other outer attributes.",
        kind.subject()
    );
    let note = format!("The outer attribute {attribute} appears before the doc comment.");
    let help = format!("Move the doc comment so it appears before {attribute} on the item.");

    FunctionAttrsMessages::new(primary, note, help)
}

/// Read the attribute spelling at its source span or return a fallback label.
fn attribute_label(cx: &LateContext<'_>, span: Span, localizer: &Localizer) -> String {
    cx.sess().source_map().span_to_snippet(span).map_or_else(
        |_| attribute_fallback(localizer),
        |snippet| snippet.trim().to_owned(),
    )
}

/// Resolve the shared fallback label for an unavailable attribute snippet.
fn attribute_fallback(lookup: &impl BundleLookup) -> String {
    let args: Arguments<'static> = Arguments::default();

    lookup
        .message(MessageKey::new("common-attribute-fallback"), &args)
        .unwrap_or_else(|_| "the preceding attribute".to_owned())
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
#[path = "tests/localization.rs"]
mod localization;

#[cfg(test)]
#[path = "tests/order_detection.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/ui.rs"]
mod ui;
