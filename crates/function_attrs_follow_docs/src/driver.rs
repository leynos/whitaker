//! Lint crate enforcing doc comment placement for functions and methods.
//!
//! The lint ensures that doc comments appear before other outer attributes on
//! free functions, inherent methods, and trait methods. Keeping doc comments at
//! the front mirrors idiomatic Rust style and prevents them from being obscured
//! by implementation details such as `#[inline]` or `#[allow]` attributes.
use rustc_ast::AttrStyle;
use rustc_ast::attr::AttributeExt;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use std::borrow::Cow;
use whitaker::{SharedConfig, recover_user_editable_hir_span};
use whitaker_common::i18n::{
    Arguments, BundleLookup, DiagnosticMessageSet, FluentValue, Localizer, MessageKey,
    MessageResolution, get_localizer_for_lint, noop_reporter, safe_resolve_message_set,
};
#[cfg(test)]
use whitaker_common::i18n::{I18nError, resolve_message_set};

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
            self.check_item_attributes(
                cx,
                ItemInfo::new(item.hir_id(), item.span, FunctionKind::Function),
            );
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        if let hir::ImplItemKind::Fn(..) = item.kind {
            self.check_item_attributes(
                cx,
                ItemInfo::new(item.hir_id(), item.span, FunctionKind::Method),
            );
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        if let hir::TraitItemKind::Fn(..) = item.kind {
            self.check_item_attributes(
                cx,
                ItemInfo::new(item.hir_id(), item.span, FunctionKind::TraitMethod),
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
    fn new(hir_id: hir::HirId, span: Span, kind: FunctionKind) -> Self {
        Self { hir_id, span, kind }
    }
}

impl<'tcx> FunctionAttrsFollowDocs {
    fn check_item_attributes(&self, cx: &LateContext<'tcx>, item: ItemInfo) {
        let attrs = cx.tcx.hir_attrs(item.hir_id);
        check_function_attributes(FunctionAttributeCheck {
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

struct AttrInfo {
    span: Span,
    user_editable_span: Option<Span>,
    is_doc: bool,
    is_outer: bool,
}

impl AttrInfo {
    /// Try to create attribute info from an HIR attribute.
    ///
    /// Returns `None` for compiler-generated attributes that don't correspond
    /// to user-written code (e.g., inline hints from derive macros).
    ///
    /// # Behaviour
    ///
    /// User-written attributes are represented as `Unparsed` (regular attributes
    /// like `#[inline]` or `#[allow(...)]`) or `DocComment` (doc comments like
    /// `///` or `//!`). These have source spans pointing to actual code locations
    /// and are processed by this lint.
    ///
    /// `Parsed` variants with a recoverable user-written span (for example
    /// `Inline` and `MustUse`; see `parsed_attribute_span` for the full
    /// whitelist) are processed like their unparsed equivalents, so
    /// attributes the compiler eagerly parses still participate in
    /// ordering. Parsed kinds without a recoverable span return `None`
    /// and are excluded: they are compiler-internal summaries whose
    /// locations would produce misleading diagnostics.
    ///
    /// See `ui/pass_derive_macro_generated.rs` for the regression test covering
    /// compiler-generated attribute handling.
    fn try_from_hir(attr: &hir::Attribute) -> Option<Self> {
        // User-written attributes are Unparsed, or a parsed AttributeKind
        // (including DocComment) whose original attribute span is
        // recoverable. Parsed kinds without a recoverable span (for
        // example `#[cold]`) cannot participate in ordering.
        let span = match attr {
            hir::Attribute::Unparsed(item) => item.span,
            hir::Attribute::Parsed(kind) => parsed_attribute_span(kind)?,
        };

        // Dummy spans indicate compiler-generated code without source location.
        if span.is_dummy() {
            return None;
        }

        let is_doc = attr.doc_str().is_some();
        let is_outer = attr
            .doc_resolution_scope()
            .is_none_or(|style| matches!(style, AttrStyle::Outer));
        let user_editable_span = recover_user_editable_hir_span(span);

        Some(Self {
            span,
            user_editable_span,
            is_doc,
            is_outer,
        })
    }

    /// Returns a source-order key using callsite spans for macro expansions.
    ///
    /// This normalizes the locations so reordered HIR attributes sort by the
    /// original source positions.
    fn source_order_key(&self) -> (rustc_span::BytePos, rustc_span::BytePos) {
        let span = self.user_editable_span.unwrap_or(self.span);
        (span.lo(), span.hi())
    }

    fn user_editable_span(&self) -> Option<Span> {
        self.user_editable_span
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

/// Context for checking function attributes.
struct FunctionAttributeCheck<'tcx, 'a> {
    cx: &'a LateContext<'tcx>,
    attrs: &'a [hir::Attribute],
    item_span: Span,
    kind: FunctionKind,
    localizer: &'a Localizer,
}

fn check_function_attributes(check: FunctionAttributeCheck<'_, '_>) {
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
    infos.sort_by_key(|info| info.source_order_key());

    let Some((doc_index, offending_index)) = detect_misordered_doc(infos.as_slice()) else {
        return;
    };

    let doc = &infos[doc_index];
    let offending = &infos[offending_index];
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
    let Some(attribute_span) = attribute_span else {
        return false;
    };

    if raw_item_span.is_dummy() {
        return true;
    }

    let item_span = item_span.unwrap_or(raw_item_span);

    // Modern nightlies exclude attributes from the item span, so outer
    // attributes sit immediately before it. Accept spans contained in the
    // item (older behaviour and inner attributes) or preceding it (outer
    // attributes on current nightlies).
    let contained = attribute_span.lo() >= item_span.lo() && attribute_span.hi() <= item_span.hi();
    let precedes = attribute_span.hi() <= item_span.lo();
    contained || precedes
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
    let messages = safe_resolve_message_set(localizer, resolution, noop_reporter, {
        let kind = context.kind;
        move || fallback_messages(kind, attribute.as_str())
    });
    let primary = messages.primary().to_string();
    let note = messages.note().to_string();
    let help = messages.help().to_string();

    cx.emit_span_lint(
        FUNCTION_ATTRS_FOLLOW_DOCS,
        context.doc_span,
        rustc_lint::errors::DiagDecorator(move |lint| {
            lint.primary_message(primary);
            lint.span_note(context.offending_span, note);
            lint.help(help);
        }),
    );
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

/// Recover the source span of a parsed attribute kind.
///
/// rustc migrates built-in attributes from `Unparsed` to parsed
/// `AttributeKind` variants nightly by nightly. There is no uniform span
/// accessor on the parsed representation, so the user-visible span is
/// recovered per kind via this whitelist. Kinds outside the whitelist
/// return `None` and are excluded from ordering checks: some carry no
/// span at all (for example `Cold` and `Used`), while others (such as
/// `AllowInternalUnsafe` and `Deprecated`) do carry a span but are
/// deliberately not recovered until the ordering check needs them. Only
/// variants whose shape is identical on the currently supported nightlies
/// are matched; further kinds can be added as the pin advances.
fn parsed_attribute_span(kind: &AttributeKind) -> Option<Span> {
    match kind {
        AttributeKind::DocComment { span, .. }
        | AttributeKind::Ignore { span, .. }
        | AttributeKind::Inline(_, span)
        | AttributeKind::MustUse { span, .. }
        | AttributeKind::Naked(span)
        | AttributeKind::NoMangle(span)
        | AttributeKind::Optimize(_, span)
        | AttributeKind::TargetFeature {
            attr_span: span, ..
        }
        | AttributeKind::TrackCaller(span) => Some(*span),
        _ => None,
    }
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
