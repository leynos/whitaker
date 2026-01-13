//! Lint crate enforcing doc comment placement for functions and methods.
//!
//! The lint ensures that doc comments appear before other outer attributes on
//! free functions, inherent methods, and trait methods. Keeping doc comments at
//! the front mirrors idiomatic Rust style and prevents them from being obscured
//! by implementation details such as `#[inline]` or `#[allow]` attributes.
use common::i18n::{
    Arguments, BundleLookup, DiagnosticMessageSet, FluentValue, Localizer, MessageKey,
    MessageResolution, get_localizer_for_lint, safe_resolve_message_set,
};
#[cfg(test)]
use common::i18n::{I18nError, resolve_message_set};
use rustc_ast::AttrStyle;
use rustc_ast::attr::AttributeExt;
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind;
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
    /// Other `Parsed` variants (Inline, Coverage, MustUse, etc.) represent
    /// compiler-internal information derived from user attributes or generated
    /// by macros. These don't have source spans corresponding to user-written
    /// code and would produce misleading diagnostics if included, so we filter
    /// them out.
    ///
    /// See `ui/pass_derive_macro_generated.rs` for the regression test covering
    /// compiler-generated attribute handling.
    fn try_from_hir(attr: &hir::Attribute) -> Option<Self> {
        // User-written attributes are Unparsed or DocComment; other Parsed
        // variants are compiler-internal and lack meaningful source locations.
        let span = match attr {
            hir::Attribute::Unparsed(item) => item.span,
            hir::Attribute::Parsed(AttributeKind::DocComment { span, .. }) => *span,
            hir::Attribute::Parsed(_) => return None,
        };

        // Dummy spans indicate compiler-generated code without source location.
        if span.is_dummy() {
            return None;
        }

        let is_doc = attr.doc_str().is_some();
        let is_outer = attr
            .doc_resolution_scope()
            .is_none_or(|style| matches!(style, AttrStyle::Outer));

        Some(Self {
            span,
            is_doc,
            is_outer,
        })
    }

    /// Returns a source-order key using callsite spans for macro expansions.
    ///
    /// This normalises the locations so reordered HIR attributes sort by the
    /// original source positions.
    fn source_order_key(&self) -> (rustc_span::BytePos, rustc_span::BytePos) {
        let span = self.span.source_callsite();
        (span.lo(), span.hi())
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
    let mut infos: Vec<AttrInfo> = check
        .attrs
        .iter()
        .filter_map(AttrInfo::try_from_hir)
        .collect();
    infos.retain(|info| attribute_within_item(info.span(), check.item_span));
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
/// Dummy spans are treated as in-bounds, and callsite spans are used to
/// normalise macro expansion locations.
fn attribute_within_item(attribute_span: Span, item_span: Span) -> bool {
    if item_span.is_dummy() {
        return true;
    }

    let attribute_span = attribute_span.source_callsite();
    let item_span = item_span.source_callsite();
    attribute_span.lo() >= item_span.lo() && attribute_span.hi() <= item_span.hi()
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
    let messages = safe_resolve_message_set(localizer, resolution, |_message| {}, {
        let kind = context.kind;
        move || fallback_messages(kind, attribute.as_str())
    });
    let primary = messages.primary().to_string();
    let note = messages.note().to_string();
    let help = messages.help().to_string();

    cx.span_lint(FUNCTION_ATTRS_FOLLOW_DOCS, context.doc_span, move |lint| {
        lint.primary_message(primary.clone());
        lint.span_note(context.offending_span, note.clone());
        lint.help(help.clone());
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
