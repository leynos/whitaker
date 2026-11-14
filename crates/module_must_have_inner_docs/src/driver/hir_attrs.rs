//! HIR attribute extraction for the module documentation check.
//!
//! The compiler has already parsed attributes when the lint runs, so the lint
//! only needs to ask which attributes a module carries and where they were
//! written. Attribute order in HIR is not source order: expansion can insert
//! trace entries and reorder expanded attributes, so the records are sorted by
//! span before the first one is classified.

use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::Span;

use super::ModuleDocDisposition;

/// A module attribute reduced to the facts the lint classifies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ModuleAttr {
    /// Whether the compiler recognizes this attribute as documentation.
    pub(super) is_doc: bool,
    /// The source span used for ordering and for the diagnostic.
    pub(super) span: Span,
}

/// Collect the recoverable attributes written inside a module body.
///
/// Attributes outside `inner_span` belong to the module declaration (for
/// example `#[path]` or a lint-level attribute on `mod foo;`) and are ignored.
/// Parsed kinds whose user-written span cannot be recovered are excluded
/// rather than assigned a synthetic position.
///
/// # Examples
///
/// ```ignore
/// let attrs = module_attrs(cx, item, item_module.spans.inner_span);
/// let disposition = classify(&attrs, !item_module.spans.inner_span.is_dummy());
/// ```
pub(super) fn module_attrs(
    cx: &LateContext<'_>,
    item: &hir::Item<'_>,
    inner_span: Span,
) -> Vec<ModuleAttr> {
    cx.tcx
        .hir_attrs(item.hir_id())
        .iter()
        .filter_map(|attr| module_attr(attr, inner_span))
        .collect()
}

/// Convert one HIR attribute into a record, or drop it when unusable.
fn module_attr(attr: &hir::Attribute, inner_span: Span) -> Option<ModuleAttr> {
    let (is_doc, span) = match attr {
        hir::Attribute::Unparsed(attr_item) => (attr.doc_str().is_some(), attr_item.span),
        hir::Attribute::Parsed(kind) => (attr.doc_str().is_some(), parsed_attribute_span(kind)?),
    };

    // Expansion-derived spans point at macro input rather than at what the
    // user wrote, and synthetic spans cannot be reported meaningfully.
    if span.is_dummy() || span.from_expansion() || !span_inside(inner_span, span) {
        return None;
    }

    Some(ModuleAttr { is_doc, span })
}

/// Return the user-written span of a parsed attribute kind when one exists.
///
/// `cfg_attr` inserts `CfgAttrTrace` and `CfgTrace` entries without a usable
/// span; other parsed kinds likewise store no source location, and
/// `Attribute::span()` panics for them. Only the kinds below have a span that
/// points at the attribute the user wrote.
fn parsed_attribute_span(kind: &hir::attrs::AttributeKind) -> Option<Span> {
    match kind {
        hir::attrs::AttributeKind::DocComment { span, .. }
        | hir::attrs::AttributeKind::Deprecated { span, .. } => Some(*span),
        hir::attrs::AttributeKind::Doc(doc) => Some(doc.first_span),
        _ => None,
    }
}

/// Report whether `inner` lies within `outer`.
fn span_inside(outer: Span, inner: Span) -> bool {
    outer.lo() <= inner.lo() && inner.hi() <= outer.hi()
}

/// Classify the module body from its extracted attribute records.
///
/// A module with no inspectable body is `Unknown`. A module with no doc record
/// is `MissingDocs`, even when other attributes are present, so the diagnostic
/// points at the module start. Otherwise the earliest attribute decides:
/// `HasLeadingDoc` when it is a doc record, and `FirstInnerIsNotDoc` carrying
/// that attribute's span when it is not.
///
/// # Examples
///
/// ```ignore
/// let records = [
///     ModuleAttr { is_doc: false, span: lint_level },
///     ModuleAttr { is_doc: true, span: doc },
/// ];
/// assert_eq!(
///     classify(&records, true),
///     ModuleDocDisposition::FirstInnerIsNotDoc(lint_level)
/// );
/// ```
pub(super) fn classify(attrs: &[ModuleAttr], has_body: bool) -> ModuleDocDisposition {
    if !has_body {
        return ModuleDocDisposition::Unknown;
    }

    if !attrs.iter().any(|attr| attr.is_doc) {
        return ModuleDocDisposition::MissingDocs;
    }

    let Some(earliest) = attrs.iter().min_by_key(|attr| attr.span.lo()) else {
        return ModuleDocDisposition::MissingDocs;
    };

    if earliest.is_doc {
        ModuleDocDisposition::HasLeadingDoc
    } else {
        ModuleDocDisposition::FirstInnerIsNotDoc(earliest.span)
    }
}
