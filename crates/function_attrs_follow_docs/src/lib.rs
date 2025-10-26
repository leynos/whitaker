//! Lint crate enforcing doc comment placement for functions and methods.
//!
//! The lint ensures that doc comments appear before other outer attributes on
//! free functions, inherent methods, and trait methods. Keeping doc comments at
//! the front mirrors idiomatic Rust style and prevents them from being obscured
//! by implementation details such as `#[inline]` or `#[allow]` attributes.
#![feature(rustc_private)]

use common::i18n::{
    Arguments, BundleLookup, DiagnosticMessageSet, FluentValue, I18nError, Localiser,
    resolve_message_set,
};
use rustc_ast::AttrStyle;
use rustc_hir as hir;
use rustc_hir::Attribute;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use std::borrow::Cow;

/// Lint pass that validates the ordering of doc comments on functions and methods.
pub struct FunctionAttrsFollowDocs {
    localiser: Localiser,
}

impl Default for FunctionAttrsFollowDocs {
    fn default() -> Self {
        Self {
            localiser: Localiser::new(None),
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
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Fn { .. } = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::Function, &self.localiser);
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        if let hir::ImplItemKind::Fn(..) = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::Method, &self.localiser);
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        if let hir::TraitItemKind::Fn(..) = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            check_function_attributes(cx, attrs, FunctionKind::TraitMethod, &self.localiser);
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
        let is_outer = matches!(attr.style(), AttrStyle::Outer);

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
    localiser: &Localiser,
) {
    let infos: Vec<AttrInfo> = attrs.iter().map(AttrInfo::from_hir).collect();

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
    emit_diagnostic(cx, context, localiser);
}

struct DiagnosticContext {
    doc_span: Span,
    offending_span: Span,
    kind: FunctionKind,
}

fn emit_diagnostic(cx: &LateContext<'_>, context: DiagnosticContext, localiser: &Localiser) {
    let attribute = attribute_label(cx, context.offending_span, localiser);
    let messages =
        localised_messages(localiser, context.kind, attribute.as_str()).unwrap_or_else(|error| {
            cx.sess().delay_span_bug(
                context.doc_span,
                format!("missing localisation for `function_attrs_follow_docs`: {error}"),
            );
            fallback_messages(context.kind, attribute.as_str())
        });

    cx.span_lint(FUNCTION_ATTRS_FOLLOW_DOCS, context.doc_span, |lint| {
        let FunctionAttrsMessages {
            primary,
            note,
            help,
        } = messages;

        lint.primary_message(primary);
        lint.span_note(context.offending_span, note);
        lint.help(help);
    });
}

const MESSAGE_KEY: &str = "function_attrs_follow_docs";

type FunctionAttrsMessages = DiagnosticMessageSet;

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

fn attribute_label(cx: &LateContext<'_>, span: Span, localiser: &Localiser) -> String {
    match cx.sess().source_map().span_to_snippet(span) {
        Ok(snippet) => snippet.trim().to_string(),
        Err(_) => attribute_fallback(localiser),
    }
}

fn attribute_fallback(lookup: &impl BundleLookup) -> String {
    let args: Arguments<'static> = Arguments::default();

    lookup
        .message("common-attribute-fallback", &args)
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
mod localisation {
    use super::{
        Arguments, BundleLookup, FunctionAttrsMessages, FunctionKind, Localiser, MESSAGE_KEY,
        localised_messages,
    };
    use common::i18n::I18nError;
    use rstest::fixture;
    use rstest_bdd_macros::{given, scenario, then, when};
    use std::cell::RefCell;

    #[derive(Default)]
    struct LocalisationWorld {
        localiser: RefCell<Option<Localiser>>,
        subject: RefCell<FunctionKind>,
        attribute: RefCell<String>,
        failing: RefCell<bool>,
        result: RefCell<Option<Result<FunctionAttrsMessages, I18nError>>>,
    }

    impl LocalisationWorld {
        fn use_localiser(&self, locale: &str) {
            let localiser = Localiser::new(Some(locale));
            *self.localiser.borrow_mut() = Some(localiser);
        }

        fn record_result(&self, value: Result<FunctionAttrsMessages, I18nError>) {
            *self.result.borrow_mut() = Some(value);
        }

        fn messages(&self) -> &FunctionAttrsMessages {
            self.result
                .borrow()
                .as_ref()
                .expect("result recorded")
                .as_ref()
                .expect("expected localisation to succeed")
        }

        fn error(&self) -> &I18nError {
            self.result
                .borrow()
                .as_ref()
                .expect("result recorded")
                .as_ref()
                .expect_err("expected localisation to fail")
        }
    }

    #[fixture]
    fn world() -> LocalisationWorld {
        LocalisationWorld::default()
    }

    #[given("the locale {locale} is selected")]
    fn given_locale(world: &LocalisationWorld, locale: String) {
        world.use_localiser(&locale);
    }

    #[given("the subject kind is {kind}")]
    fn given_subject(world: &LocalisationWorld, kind: String) {
        *world.subject.borrow_mut() = match kind.as_str() {
            "function" => FunctionKind::Function,
            "method" => FunctionKind::Method,
            "trait method" => FunctionKind::TraitMethod,
            other => panic!("unknown subject kind: {other}"),
        };
    }

    #[given("the attribute label is {label}")]
    fn given_attribute(world: &LocalisationWorld, label: String) {
        *world.attribute.borrow_mut() = label;
    }

    #[given("localisation fails")]
    fn given_failure(world: &LocalisationWorld) {
        *world.failing.borrow_mut() = true;
    }

    #[when("I localise the diagnostic")]
    fn when_localise(world: &LocalisationWorld) {
        let attribute = world.attribute.borrow().clone();
        let kind = *world.subject.borrow();

        let result = if *world.failing.borrow() {
            localised_messages(&FailingLookup, kind, attribute.as_str())
        } else {
            let localiser = world
                .localiser
                .borrow()
                .as_ref()
                .expect("a locale must be selected");
            localised_messages(localiser, kind, attribute.as_str())
        };

        world.record_result(result);
    }

    #[then("the primary message contains {snippet}")]
    fn then_primary(world: &LocalisationWorld, snippet: String) {
        assert!(world.messages().primary().contains(&snippet));
    }

    #[then("the note mentions {snippet}")]
    fn then_note(world: &LocalisationWorld, snippet: String) {
        assert!(world.messages().note().contains(&snippet));
    }

    #[then("the help mentions {snippet}")]
    fn then_help(world: &LocalisationWorld, snippet: String) {
        assert!(world.messages().help().contains(&snippet));
    }

    #[then("localisation fails for {key}")]
    fn then_failure(world: &LocalisationWorld, key: String) {
        let error = world.error();
        match error {
            I18nError::MissingMessage { key: missing, .. } => assert_eq!(missing, &key),
        }
    }

    #[scenario(path = "tests/features/function_attrs_localisation.feature", index = 0)]
    fn scenario_fallback(world: LocalisationWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/function_attrs_localisation.feature", index = 1)]
    fn scenario_welsh(world: LocalisationWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/function_attrs_localisation.feature", index = 2)]
    fn scenario_unknown_locale(world: LocalisationWorld) {
        let _ = world;
    }

    #[scenario(path = "tests/features/function_attrs_localisation.feature", index = 3)]
    fn scenario_failure(world: LocalisationWorld) {
        let _ = world;
    }

    struct FailingLookup;

    impl BundleLookup for FailingLookup {
        fn message(&self, _key: &str, _args: &Arguments<'_>) -> Result<String, I18nError> {
            Err(I18nError::MissingMessage {
                key: MESSAGE_KEY.to_string(),
                locale: "test".to_string(),
            })
        }

        fn attribute(
            &self,
            _key: &str,
            _attribute: &str,
            _args: &Arguments<'_>,
        ) -> Result<String, I18nError> {
            Err(I18nError::MissingMessage {
                key: MESSAGE_KEY.to_string(),
                locale: "test".to_string(),
            })
        }
    }
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
