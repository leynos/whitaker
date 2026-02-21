//! Lint crate enforcing example-free documentation for test functions.

use crate::heuristics::{DocExampleViolation, detect_example_violation};
use common::AttributePath;
use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    get_localizer_for_lint, noop_reporter, safe_resolve_message_set,
};
use log::debug;
use rustc_hir as hir;
use rustc_hir::Node;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Ident, Span, Symbol};
use serde::Deserialize;
use std::borrow::Cow;
use whitaker::SharedConfig;
use whitaker::hir::has_test_like_hir_attributes;

const LINT_NAME: &str = "test_must_not_have_example";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new("test_must_not_have_example");

#[derive(Default, Deserialize)]
struct Config {
    #[serde(default)]
    additional_test_attributes: Vec<String>,
}

dylint_linting::impl_late_lint! {
    pub TEST_MUST_NOT_HAVE_EXAMPLE,
    Warn,
    "test functions should not include examples or fenced code in documentation",
    TestMustNotHaveExample::default()
}

/// Lint pass that checks test documentation for example sections.
pub struct TestMustNotHaveExample {
    /// Additional attribute paths configured as test-like markers.
    additional_test_attributes: Vec<AttributePath>,
    /// Localized message resolver used for emitted diagnostics.
    localizer: Localizer,
}

struct FunctionSite<'a> {
    name: &'a str,
    span: Span,
}

enum ItemKindInfo<'a> {
    Item {
        ident: Ident,
        attrs: &'a [hir::Attribute],
    },
    ImplItem {
        ident: &'a Ident,
        attrs: &'a [hir::Attribute],
    },
    TraitItem {
        ident: &'a Ident,
        attrs: &'a [hir::Attribute],
    },
}

impl<'a> ItemKindInfo<'a> {
    fn ident(&self) -> &Ident {
        match self {
            ItemKindInfo::Item { ident, .. } => ident,
            ItemKindInfo::ImplItem { ident, .. } | ItemKindInfo::TraitItem { ident, .. } => ident,
        }
    }

    fn attrs(&self) -> &'a [hir::Attribute] {
        match self {
            ItemKindInfo::Item { attrs, .. }
            | ItemKindInfo::ImplItem { attrs, .. }
            | ItemKindInfo::TraitItem { attrs, .. } => attrs,
        }
    }
}

/// Macro to implement check methods for function-bearing HIR items.
macro_rules! impl_check_method {
    ($method_name:ident, $item_type:ty, $kind_pattern:pat, $variant:ident) => {
        fn $method_name(&mut self, cx: &LateContext<'tcx>, item: &'tcx $item_type) {
            if let $kind_pattern = item.kind {
                let attrs = cx.tcx.hir_attrs(item.hir_id());
                self.check_function_item(
                    cx,
                    ItemKindInfo::$variant {
                        ident: &item.ident,
                        attrs,
                    },
                    None,
                );
            }
        }
    };
}

impl Default for TestMustNotHaveExample {
    fn default() -> Self {
        Self {
            additional_test_attributes: Vec::new(),
            localizer: Localizer::new(None),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for TestMustNotHaveExample {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        let config = match dylint_linting::config::<Config>(LINT_NAME) {
            Ok(Some(config)) => config,
            Ok(None) => Config::default(),
            Err(error) => {
                debug!(
                    target: LINT_NAME,
                    "failed to parse `{}` configuration: {error}; using defaults",
                    LINT_NAME
                );
                Config::default()
            }
        };

        self.additional_test_attributes = config
            .additional_test_attributes
            .iter()
            .map(|path| AttributePath::from(path.as_str()))
            .collect();

        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Fn { .. } = item.kind {
            let Some(ident) = item.kind.ident() else {
                return;
            };
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            self.check_function_item(cx, ItemKindInfo::Item { ident, attrs }, Some(item));
        }
    }

    impl_check_method!(
        check_impl_item,
        hir::ImplItem<'tcx>,
        hir::ImplItemKind::Fn(..),
        ImplItem
    );
    impl_check_method!(
        check_trait_item,
        hir::TraitItem<'tcx>,
        hir::TraitItemKind::Fn(..),
        TraitItem
    );
}

impl TestMustNotHaveExample {
    fn detect_violation(
        &self,
        attrs: &[hir::Attribute],
        is_test: bool,
    ) -> Option<DocExampleViolation> {
        if !is_test {
            return None;
        }

        let doc_text = collect_doc_text(attrs);
        if doc_text.is_empty() {
            return None;
        }

        detect_example_violation(&doc_text)
    }

    fn emit_violation(
        &self,
        cx: &LateContext<'_>,
        function: FunctionSite<'_>,
        violation: DocExampleViolation,
    ) {
        let messages = localized_messages(&self.localizer, function.name, violation);
        let primary = messages.primary().to_string();
        let note = messages.note().to_string();
        let help = messages.help().to_string();

        cx.span_lint(TEST_MUST_NOT_HAVE_EXAMPLE, function.span, move |lint| {
            lint.primary_message(primary);
            lint.note(note);
            lint.help(help);
        });
    }

    fn check_function_item<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        item_info: ItemKindInfo<'_>,
        item: Option<&'tcx hir::Item<'tcx>>,
    ) {
        let attrs = item_info.attrs();
        let is_test = if let Some(item) = item {
            is_test_function_item(cx, item, attrs, self.additional_test_attributes.as_slice())
        } else {
            has_test_like_hir_attributes(attrs, self.additional_test_attributes.as_slice())
        };

        if let Some(violation) = self.detect_violation(attrs, is_test) {
            self.emit_violation(
                cx,
                FunctionSite {
                    name: item_info.ident().name.as_str(),
                    span: item_info.ident().span,
                },
                violation,
            );
        }
    }
}

fn is_test_function_item<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx hir::Item<'tcx>,
    attrs: &[hir::Attribute],
    additional_test_attributes: &[AttributePath],
) -> bool {
    has_test_like_hir_attributes(attrs, additional_test_attributes)
        || is_harness_marked_test_function(cx, item)
}

fn is_harness_marked_test_function<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx hir::Item<'tcx>,
) -> bool {
    if !cx.tcx.sess.opts.test {
        return false;
    }

    let Some(function_ident) = item.kind.ident() else {
        return false;
    };

    let function_hir_id = item.hir_id();
    let function_name = function_ident.name;
    let function_span = item.span;
    let mut parents = cx.tcx.hir_parent_iter(item.hir_id());
    let Some((_, parent_node)) = parents.next() else {
        return false;
    };

    match parent_node {
        Node::Item(parent_item) => {
            let hir::ItemKind::Mod(_, module) = parent_item.kind else {
                return false;
            };
            module
                .item_ids
                .iter()
                .map(|id| cx.tcx.hir_item(*id))
                .any(|sibling| {
                    is_matching_harness_test_descriptor(
                        function_hir_id,
                        function_name,
                        function_span,
                        sibling,
                    )
                })
        }
        Node::Crate(_crate_node) => cx
            .tcx
            .hir_crate_items(())
            .free_items()
            .map(|id| cx.tcx.hir_item(id))
            .any(|sibling| {
                is_matching_harness_test_descriptor(
                    function_hir_id,
                    function_name,
                    function_span,
                    sibling,
                )
            }),
        _ => false,
    }
}

fn is_matching_harness_test_descriptor(
    function_hir_id: hir::HirId,
    function_name: Symbol,
    function_span: Span,
    sibling: &hir::Item<'_>,
) -> bool {
    // The `rustc --test` harness may synthesise a const descriptor that shares
    // the test function's name and span; matching this pair lets us recover
    // test context when the original marker attribute is unavailable.
    sibling.hir_id() != function_hir_id
        && matches!(sibling.kind, hir::ItemKind::Const(..))
        && sibling
            .kind
            .ident()
            .is_some_and(|ident| ident.name == function_name && sibling.span == function_span)
}

fn collect_doc_text(attrs: &[hir::Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|attr| attr.doc_str().map(|doc| doc.to_string()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn localized_messages(
    localizer: &Localizer,
    function_name: &str,
    violation: DocExampleViolation,
) -> DiagnosticMessageSet {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("test"),
        FluentValue::from(function_name.to_string()),
    );

    let reason = violation.note_detail().to_string();
    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };
    safe_resolve_message_set(localizer, resolution, noop_reporter, move || {
        fallback_messages(function_name, reason.as_str())
    })
}

fn fallback_messages(function_name: &str, reason: &str) -> DiagnosticMessageSet {
    DiagnosticMessageSet::new(
        format!("Remove example sections from test {function_name} documentation."),
        format!("The docs for {function_name} contain {reason}."),
        String::from("Drop the example or move it into standalone user-facing documentation."),
    )
}
