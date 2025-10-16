//! Lint crate enforcing doc comment placement for functions and methods.
//!
//! The lint ensures that doc comments appear before other outer attributes on
//! free functions, inherent methods, and trait methods. Keeping doc comments at
//! the front mirrors idiomatic Rust style and prevents them from being obscured
//! by implementation details such as `#[inline]` or `#[allow]` attributes.
#![feature(rustc_private)]

use rustc_ast::AttrStyle;
use rustc_attr_data_structures::AttributeKind;
use rustc_hir as hir;
use rustc_hir::Attribute;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

/// Lint pass that validates the ordering of doc comments on functions and methods.
#[derive(Default)]
pub struct FunctionAttrsFollowDocs;

dylint_linting::impl_late_lint! {
    pub FUNCTION_ATTRS_FOLLOW_DOCS,
    Warn,
    "doc comments on functions must precede other outer attributes",
    FunctionAttrsFollowDocs::default()
}

impl<'tcx> LateLintPass<'tcx> for FunctionAttrsFollowDocs {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Fn { .. } = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::Function);
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        if let hir::ImplItemKind::Fn(..) = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::Method);
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        if let hir::TraitItemKind::Fn(..) = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::TraitMethod);
        }
    }
}

#[derive(Clone, Copy)]
enum FunctionKind {
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
    fn from_hir(attr: &Attribute) -> Self {
        let span = attr.span();
        let is_doc = attr.doc_str().is_some();
        let is_outer = match attr {
            Attribute::Parsed(AttributeKind::DocComment { style, .. }) => {
                matches!(style, AttrStyle::Outer)
            }
            Attribute::Parsed(_) => true,
            Attribute::Unparsed(item) => matches!(item.style, AttrStyle::Outer),
        };

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

fn check_function_attributes(cx: &LateContext<'_>, attrs: &[Attribute], kind: FunctionKind) {
    let infos: Vec<AttrInfo> = attrs.iter().map(AttrInfo::from_hir).collect();

    let Some((doc_index, offending_index)) = detect_misordered_doc(infos.as_slice()) else {
        return;
    };

    let doc = &infos[doc_index];
    let offending = &infos[offending_index];
    emit_diagnostic(cx, doc.span(), offending.span(), kind);
}

fn emit_diagnostic(cx: &LateContext<'_>, doc_span: Span, offending_span: Span, kind: FunctionKind) {
    cx.span_lint(FUNCTION_ATTRS_FOLLOW_DOCS, doc_span, |lint| {
        lint.primary_message(format!(
            "doc comments on {} must precede other outer attributes",
            kind.subject()
        ));
        lint.span_note(
            offending_span,
            "an outer attribute appears before the doc comment",
        );
        lint.help("Move the doc comment before other outer attributes.");
    });
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

        if attribute.is_doc() {
            if let Some(non_doc_index) = first_non_doc_outer {
                return Some((index, non_doc_index));
            }
        } else if first_non_doc_outer.is_none() {
            first_non_doc_outer = Some(index);
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
mod tests {
    use super::{OrderedAttribute, detect_misordered_doc};
    use common::attributes::{Attribute, AttributeKind, AttributePath};
    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};
    use rustc_span::{DUMMY_SP, Span};
    use std::cell::RefCell;

    impl OrderedAttribute for Attribute {
        fn is_outer(&self) -> bool {
            self.is_outer()
        }

        fn is_doc(&self) -> bool {
            self.is_doc()
        }

        fn span(&self) -> Span {
            DUMMY_SP
        }
    }

    #[derive(Default)]
    struct AttributeWorld {
        attributes: RefCell<Vec<Attribute>>,
    }

    impl AttributeWorld {
        fn push(&self, path: &str, kind: AttributeKind) {
            self.attributes
                .borrow_mut()
                .push(Attribute::new(AttributePath::from(path), kind));
        }

        fn result(&self) -> Option<(usize, usize)> {
            detect_misordered_doc(self.attributes.borrow().as_slice())
        }
    }

    #[fixture]
    fn world() -> AttributeWorld {
        AttributeWorld::default()
    }

    #[fixture]
    fn result() -> Option<(usize, usize)> {
        None
    }

    #[given("a doc comment before other attributes")]
    fn doc_precedes(world: &AttributeWorld) {
        world.push("doc", AttributeKind::Outer);
        world.push("inline", AttributeKind::Outer);
    }

    #[given("a doc comment after an attribute")]
    fn doc_follows(world: &AttributeWorld) {
        world.push("inline", AttributeKind::Outer);
        world.push("doc", AttributeKind::Outer);
    }

    #[given("attributes without doc comments")]
    fn no_doc(world: &AttributeWorld) {
        world.push("inline", AttributeKind::Outer);
        world.push("allow", AttributeKind::Outer);
    }

    #[given("a doc comment after an inner attribute")]
    fn doc_after_inner(world: &AttributeWorld) {
        world.push("test", AttributeKind::Inner);
        world.push("doc", AttributeKind::Outer);
        world.push("inline", AttributeKind::Outer);
    }

    #[when("I evaluate the attribute order")]
    fn evaluate(world: &AttributeWorld) -> Option<(usize, usize)> {
        world.result()
    }

    #[then("the order is accepted")]
    fn order_ok(result: &Option<(usize, usize)>) {
        assert!(result.is_none());
    }

    #[then("the order is rejected")]
    fn order_rejected(result: &Option<(usize, usize)>) {
        assert!(result.is_some());
    }

    #[scenario(path = "tests/features/function_doc_order.feature", index = 0)]
    fn scenario_accepts_doc_first(world: AttributeWorld, result: Option<(usize, usize)>) {
        let _ = (world, result);
    }

    #[scenario(path = "tests/features/function_doc_order.feature", index = 1)]
    fn scenario_rejects_doc_last(world: AttributeWorld, result: Option<(usize, usize)>) {
        let _ = (world, result);
    }

    #[scenario(path = "tests/features/function_doc_order.feature", index = 2)]
    fn scenario_handles_no_doc(world: AttributeWorld, result: Option<(usize, usize)>) {
        let _ = (world, result);
    }

    #[scenario(path = "tests/features/function_doc_order.feature", index = 3)]
    fn scenario_ignores_inner_attributes(world: AttributeWorld, result: Option<(usize, usize)>) {
        let _ = (world, result);
    }
}

#[cfg(test)]
mod ui {
    whitaker::declare_ui_tests!("ui");
}
