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
            let mut marked = whitaker::hir::collect_harness_test_functions(cx);
            marked.extend(whitaker::hir::collect_rstest_companion_test_functions(cx));
            marked
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

        let summary = summarise_context_with_harness(
            cx,
            expr.hir_id,
            self.is_test_harness,
            &self.harness_test_functions,
        );

        let panic_info = closure_panics(cx, body_id);
        if !should_flag(&self.policy, &summary, &panic_info, self.is_doctest) {
            return;
        }

        emit_diagnostic(cx, expr, receiver, &self.localizer);
    }
}

/// Summarizes the lint context for an expression, merging attribute-based and
/// harness-based test detection into a single immutable result.
fn summarise_context_with_harness<'tcx>(
    cx: &LateContext<'tcx>,
    hir_id: hir::HirId,
    is_test_harness: bool,
    harness_test_functions: &HashSet<hir::HirId>,
) -> ContextSummary {
    let mut summary = crate::context::summarise_context(cx, hir_id);

    // Merge harness-based test detection if not already identified as a test
    // via attributes.
    if !summary.is_test && is_test_harness {
        summary.is_test = cx.tcx.hir_parent_iter(hir_id).any(|(_, node)| match node {
            hir::Node::Item(item) if matches!(item.kind, hir::ItemKind::Fn { .. }) => {
                harness_test_functions.contains(&item.hir_id())
            }
            _ => false,
        });
    }

    summary
}

fn closure_body(expr: &hir::Expr<'_>) -> Option<hir::BodyId> {
    match expr.kind {
        ExprKind::Closure(hir::Closure { body, .. }) => Some(*body),
        _ => None,
    }
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
