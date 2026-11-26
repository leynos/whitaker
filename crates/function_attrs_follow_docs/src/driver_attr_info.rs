//! Source-order information for the attributes attached to one function item.

use rustc_ast::{AttrStyle, attr::AttributeExt};
use rustc_hir as hir;
use rustc_hir::attrs::AttributeKind;
use rustc_span::Span;
use whitaker_lint_core::recover_user_editable_hir_span;

use super::OrderedAttribute;

/// One user-written attribute, with the spans needed to order and report it.
pub(super) struct AttrInfo {
    pub(super) span: Span,
    pub(super) user_editable_span: Option<Span>,
    pub(super) is_doc: bool,
    pub(super) is_outer: bool,
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
    pub(super) fn try_from_hir(attr: &hir::Attribute) -> Option<Self> {
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

        // Attributes whose span carries an expansion context were inserted by
        // a macro rather than written at that position; the user cannot
        // reorder them, and their recovered call-site span would misattribute
        // an enclosing item's attribute (for example `#[async_trait]`) to
        // every item the macro emits.
        if span.from_expansion() {
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
    pub(super) fn source_order_key(&self) -> (rustc_span::BytePos, rustc_span::BytePos) {
        let span = self.user_editable_span.unwrap_or(self.span);
        (span.lo(), span.hi())
    }

    /// Return the span that can be edited in user-written source.
    pub(super) const fn user_editable_span(&self) -> Option<Span> { self.user_editable_span }
}

impl OrderedAttribute for AttrInfo {
    fn is_outer(&self) -> bool { self.is_outer }

    fn is_doc(&self) -> bool { self.is_doc }

    fn span(&self) -> Span { self.span }
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
pub(super) const fn parsed_attribute_span(kind: &AttributeKind) -> Option<Span> {
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
