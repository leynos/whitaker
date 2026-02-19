//! Lint crate enforcing example-free documentation for test functions.

use crate::heuristics::{DocExampleViolation, detect_example_violation};
use common::i18n::{
    Arguments, DiagnosticMessageSet, FluentValue, Localizer, MessageKey, MessageResolution,
    get_localizer_for_lint, noop_reporter, safe_resolve_message_set,
};
use common::{Attribute, AttributeKind, AttributePath};
use log::debug;
use rustc_hir as hir;
use rustc_hir::Node;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Ident, Span};
use serde::Deserialize;
use std::borrow::Cow;
use whitaker::SharedConfig;

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
    additional_test_attributes: Vec<AttributePath>,
    localizer: Localizer,
}

struct FunctionSite<'a> {
    name: &'a str,
    span: Span,
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
            let is_test =
                is_test_function_item(cx, item, attrs, self.additional_test_attributes.as_slice());
            self.check_function(cx, attrs, &ident, is_test);
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        if let hir::ImplItemKind::Fn(..) = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            let is_test =
                has_test_like_hir_attributes(attrs, self.additional_test_attributes.as_slice());
            self.check_function(cx, attrs, &item.ident, is_test);
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        if let hir::TraitItemKind::Fn(..) = item.kind {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            let is_test =
                has_test_like_hir_attributes(attrs, self.additional_test_attributes.as_slice());
            self.check_function(cx, attrs, &item.ident, is_test);
        }
    }
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
        let messages = localised_messages(&self.localizer, function.name, violation);
        let primary = messages.primary().to_string();
        let note = messages.note().to_string();
        let help = messages.help().to_string();

        cx.span_lint(TEST_MUST_NOT_HAVE_EXAMPLE, function.span, move |lint| {
            lint.primary_message(primary.clone());
            lint.note(note.clone());
            lint.help(help.clone());
        });
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "signature intentionally mirrors item-specific callers to keep test-detection logic at call sites"
    )]
    fn check_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        attrs: &[hir::Attribute],
        ident: &Ident,
        is_test: bool,
    ) {
        if let Some(violation) = self.detect_violation(attrs, is_test) {
            self.emit_violation(
                cx,
                FunctionSite {
                    name: ident.name.as_str(),
                    span: ident.span,
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

    sibling_items(cx, item)
        .into_iter()
        .filter(|sibling| sibling.hir_id() != item.hir_id())
        .filter(|sibling| matches!(sibling.kind, hir::ItemKind::Const(..)))
        .filter_map(|sibling| sibling.kind.ident().map(|ident| (ident, sibling)))
        .any(|(ident, sibling)| ident.name == function_ident.name && sibling.span == item.span)
}

fn sibling_items<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx hir::Item<'tcx>,
) -> Vec<&'tcx hir::Item<'tcx>> {
    let mut parents = cx.tcx.hir_parent_iter(item.hir_id());
    let Some((_, parent_node)) = parents.next() else {
        return Vec::new();
    };

    match parent_node {
        Node::Item(parent_item) => {
            let hir::ItemKind::Mod(_, module) = parent_item.kind else {
                return Vec::new();
            };
            module
                .item_ids
                .iter()
                .map(|id| cx.tcx.hir_item(*id))
                .collect()
        }
        Node::Crate(_crate_node) => cx
            .tcx
            .hir_crate_items(())
            .free_items()
            .map(|id| cx.tcx.hir_item(id))
            .collect(),
        _ => Vec::new(),
    }
}

fn has_test_like_hir_attributes(attrs: &[hir::Attribute], additional: &[AttributePath]) -> bool {
    attrs
        .iter()
        .filter_map(attribute_path)
        .any(|path| Attribute::new(path, AttributeKind::Outer).is_test_like_with(additional))
}

fn attribute_path(attr: &hir::Attribute) -> Option<AttributePath> {
    let hir::Attribute::Unparsed(_) = attr else {
        return None;
    };

    let mut names = attr.path().into_iter().map(|symbol| symbol.to_string());
    let first = names.next()?;
    Some(AttributePath::new(std::iter::once(first).chain(names)))
}

fn collect_doc_text(attrs: &[hir::Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|attr| attr.doc_str().map(|doc| doc.to_string()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn localised_messages(
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
        format!("Remove example sections from test `{function_name}` documentation."),
        format!("The docs for `{function_name}` contain {reason}."),
        String::from("Drop the example or move it into standalone user-facing documentation."),
    )
}
