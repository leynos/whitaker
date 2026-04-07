//! Lint wiring that flags panicking `unwrap_or_else` fallbacks.

use crate::LINT_NAME;
use crate::context::ContextSummary;
use crate::diagnostics::emit_diagnostic;
use crate::panic_detector::{closure_panics, receiver_is_option_or_result};
use crate::policy::{LintPolicy, should_flag};
use log::debug;
use rustc_hir as hir;
use rustc_hir::ExprKind;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Symbol;
use serde::Deserialize;
use std::collections::HashSet;
use whitaker::SharedConfig;
use whitaker_common::i18n::{Localizer, get_localizer_for_lint};

dylint_linting::impl_late_lint! {
    pub NO_UNWRAP_OR_ELSE_PANIC,
    Deny,
    "forbid `unwrap_or_else` whose closure panics (directly or via unwrap/expect)",
    NoUnwrapOrElsePanic::default()
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    allow_in_main: Option<bool>,
}

impl Config {
    fn resolved_allow_in_main(&self) -> bool {
        self.allow_in_main.unwrap_or(false)
    }
}

/// Lint pass that inspects `unwrap_or_else` fallbacks for panics.
pub struct NoUnwrapOrElsePanic {
    policy: LintPolicy,
    localizer: Localizer,
    is_doctest: bool,
    is_test_harness: bool,
    harness_test_functions: HashSet<hir::HirId>,
}

impl Default for NoUnwrapOrElsePanic {
    fn default() -> Self {
        Self {
            policy: LintPolicy::default(),
            localizer: Localizer::new(None),
            is_doctest: false,
            is_test_harness: false,
            harness_test_functions: HashSet::new(),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for NoUnwrapOrElsePanic {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.is_doctest = cx
            .tcx
            .env_var_os("UNSTABLE_RUSTDOC_TEST_PATH".as_ref())
            .is_some();

        self.is_test_harness = cx.tcx.sess.opts.test;
        self.harness_test_functions = if self.is_test_harness {
            collect_harness_test_functions(cx)
        } else {
            HashSet::new()
        };

        let config = load_configuration();
        self.policy = LintPolicy::new(config.resolved_allow_in_main());

        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        let ExprKind::MethodCall(segment, receiver, args, _) = expr.kind else {
            return;
        };

        if segment.ident.name.as_str() != "unwrap_or_else" {
            return;
        }

        if !receiver_is_option_or_result(cx, receiver) {
            return;
        }

        let Some(fallback) = args.first() else {
            return;
        };

        let Some(body_id) = closure_body(fallback) else {
            return;
        };

        let mut summary = summarise_context(cx, expr.hir_id);
        if !summary.is_test && self.is_test_harness {
            summary.is_test =
                is_inside_harness_test_function(cx, expr, &self.harness_test_functions);
        }

        let panic_info = closure_panics(cx, body_id);
        if !should_flag(&self.policy, &summary, &panic_info, self.is_doctest) {
            return;
        }

        emit_diagnostic(cx, expr, receiver, &self.localizer);
    }
}

fn summarise_context<'tcx>(cx: &LateContext<'tcx>, hir_id: hir::HirId) -> ContextSummary {
    crate::context::summarise_context(cx, hir_id)
}

fn closure_body(expr: &hir::Expr<'_>) -> Option<hir::BodyId> {
    match expr.kind {
        ExprKind::Closure(hir::Closure { body, .. }) => Some(*body),
        _ => None,
    }
}

/// Returns `true` when `expr` is inside a function that the `--test` harness
/// has marked as a test entry point.
fn is_inside_harness_test_function<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &hir::Expr<'tcx>,
    harness_test_functions: &HashSet<hir::HirId>,
) -> bool {
    cx.tcx
        .hir_parent_iter(expr.hir_id)
        .any(|(_, node)| match node {
            hir::Node::Item(item) if matches!(item.kind, hir::ItemKind::Fn { .. }) => {
                harness_test_functions.contains(&item.hir_id())
            }
            _ => false,
        })
}

/// Collects all functions that the `rustc --test` harness identifies as tests.
///
/// The test harness synthesises a sibling `const` descriptor with the same name
/// and source span as each test function. This function scans for those
/// descriptors to recover test-function identity after the harness has consumed
/// the original `#[test]` attributes.
fn collect_harness_test_functions<'tcx>(cx: &LateContext<'tcx>) -> HashSet<hir::HirId> {
    let root_items: Vec<_> = cx
        .tcx
        .hir_crate_items(())
        .free_items()
        .map(|id| cx.tcx.hir_item(id))
        .collect();
    let mut marked = HashSet::new();
    collect_in_item_group(cx, &root_items, &mut marked);
    marked
}

fn collect_in_item_group<'tcx>(
    cx: &LateContext<'tcx>,
    items: &[&'tcx hir::Item<'tcx>],
    marked: &mut HashSet<hir::HirId>,
) {
    for item in items
        .iter()
        .copied()
        .filter(|item| matches!(item.kind, hir::ItemKind::Fn { .. }))
    {
        let Some(ident) = item.kind.ident() else {
            continue;
        };

        if items
            .iter()
            .any(|sibling| is_test_descriptor(item.hir_id(), ident.name, item.span, sibling))
        {
            marked.insert(item.hir_id());
        }
    }

    // Recurse into submodules.
    for item in items {
        let hir::ItemKind::Mod(_, module) = item.kind else {
            continue;
        };
        let module_items: Vec<_> = module
            .item_ids
            .iter()
            .map(|id| cx.tcx.hir_item(*id))
            .collect();
        collect_in_item_group(cx, &module_items, marked);
    }
}

/// The `--test` harness synthesises a `const` with the same name and source
/// range as the test function.
fn is_test_descriptor(
    fn_hir_id: hir::HirId,
    fn_name: Symbol,
    fn_span: rustc_span::Span,
    sibling: &hir::Item<'_>,
) -> bool {
    sibling.hir_id() != fn_hir_id
        && matches!(sibling.kind, hir::ItemKind::Const(..))
        && sibling
            .kind
            .ident()
            .is_some_and(|ident| ident.name == fn_name && sibling.span.source_equal(fn_span))
}

fn load_configuration() -> Config {
    match dylint_linting::config::<Config>(LINT_NAME) {
        Ok(Some(config)) => config,
        Ok(None) => Config::default(),
        Err(error) => {
            debug!(
                target: LINT_NAME,
                "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
            );
            Config::default()
        }
    }
}
