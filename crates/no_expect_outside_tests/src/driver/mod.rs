//! Lint crate forbidding `.expect(..)` outside test and doctest contexts.
//!
//! The lint inspects method calls named `expect`, verifies that the receiver
//! is an `Option` or `Result`, and checks the surrounding traversal context for
//! test-like attributes or `cfg(test)` guards. Doctest harnesses are skipped via
//! `Crate::is_doctest`, ensuring documentation examples remain ergonomic. When
//! no test context is present, the lint emits a denial with a note describing
//! the enclosing function and the receiver type to guide remediation. Teams can
//! extend the recognised test attributes through `dylint.toml` when bespoke
//! macros are in play.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Component;

use log::debug;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol, sym};
use serde::Deserialize;
use whitaker::SharedConfig;
use whitaker::hir::has_test_like_hir_attributes;
use whitaker_common::{AttributePath, Localizer, get_localizer_for_lint};

use crate::context::{collect_context, summarise_context};
use crate::diagnostics::{DiagnosticContext, emit_diagnostic};

dylint_linting::impl_late_lint! {
    pub NO_EXPECT_OUTSIDE_TESTS,
    Deny,
    "`.expect(..)` must not be used outside of test or doctest contexts",
    NoExpectOutsideTests::default()
}

#[derive(Default, Deserialize)]
struct Config {
    #[serde(default)]
    additional_test_attributes: Vec<String>,
}

/// Lint pass that tracks contexts while checking method calls.
pub struct NoExpectOutsideTests {
    is_doctest: bool,
    is_test_harness: bool,
    additional_test_attributes: Vec<AttributePath>,
    harness_marked_test_functions: HashSet<hir::HirId>,
    localizer: Localizer,
}

impl Default for NoExpectOutsideTests {
    fn default() -> Self {
        Self {
            is_doctest: false,
            is_test_harness: false,
            additional_test_attributes: Vec::new(),
            harness_marked_test_functions: HashSet::new(),
            localizer: Localizer::new(None),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for NoExpectOutsideTests {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.is_doctest = cx
            .tcx
            .env_var_os("UNSTABLE_RUSTDOC_TEST_PATH".as_ref())
            .is_some();
        self.is_test_harness = cx.tcx.sess.opts.test;
        self.harness_marked_test_functions = if self.is_test_harness {
            collect_harness_marked_test_functions(cx)
        } else {
            HashSet::new()
        };
        let config_name = "no_expect_outside_tests";
        let config = match dylint_linting::config::<Config>(config_name) {
            Ok(Some(config)) => config,
            Ok(None) => {
                debug!(
                    target: config_name,
                    "no configuration found for `{config_name}`; using defaults"
                );
                Config::default()
            }
            Err(error) => {
                debug!(
                    target: config_name,
                    "failed to parse `{config_name}` configuration: {error}; using defaults"
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
        self.localizer = get_localizer_for_lint("no_expect_outside_tests", shared_config.locale());
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        if self.is_doctest {
            return;
        }

        let hir::ExprKind::MethodCall(segment, receiver, ..) = expr.kind else {
            return;
        };

        if segment.ident.name != sym::expect {
            return;
        }

        if !receiver_is_option_or_result(cx, receiver) {
            return;
        }

        let additional = self.additional_test_attributes.as_slice();
        let (entries, has_cfg_test) = collect_context(cx, expr.hir_id, additional);
        let summary = summarise_context(entries.as_slice(), has_cfg_test, additional);

        if summary.is_test {
            return;
        }

        // Fallback: when compiled with --test (integration test crates), functions
        // with #[test] may not be detected via attributes if the test framework
        // processes them differently. Allow expect() in functions that appear to
        // be tests based on the harness context.
        if self.is_test_harness
            && is_likely_test_function(cx, expr, &self.harness_marked_test_functions)
        {
            return;
        }

        let diagnostic_context = DiagnosticContext::new(&summary, &self.localizer);
        emit_diagnostic(cx, expr, receiver, &diagnostic_context);
    }
}

fn receiver_is_option_or_result<'tcx>(
    cx: &LateContext<'tcx>,
    receiver: &'tcx hir::Expr<'tcx>,
) -> bool {
    let ty = cx.typeck_results().expr_ty(receiver);

    ty_is_option_or_result(cx, ty)
}

fn ty_is_option_or_result<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    let typing_env = ty::TypingEnv {
        typing_mode: ty::TypingMode::non_body_analysis(),
        param_env: cx.param_env,
    };
    let ty = cx.tcx.normalize_erasing_regions(typing_env, ty).peel_refs();

    let Some(adt) = ty.ty_adt_def() else {
        return false;
    };

    let def_id = adt.did();
    cx.tcx.is_diagnostic_item(sym::Option, def_id) || cx.tcx.is_diagnostic_item(sym::Result, def_id)
}

// Check if the expression is inside a function that appears to be a test.
//
// This is a fallback for when the standard attribute detection doesn't find
// #[test] (which may happen in integration test crates where the test harness
// processes attributes differently).
fn is_likely_test_function<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &hir::Expr<'tcx>,
    harness_marked_test_functions: &HashSet<hir::HirId>,
) -> bool {
    if cx
        .tcx
        .hir_parent_iter(expr.hir_id)
        .filter_map(|(_, node)| extract_function_item(node))
        .any(|item| {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            has_test_attribute(attrs)
                || is_harness_marked_test_function(item.hir_id(), harness_marked_test_functions)
        })
    {
        return true;
    }

    // Check if we're inside a module named "tests" (common convention for unit tests)
    let in_test_module = cx
        .tcx
        .hir_parent_iter(expr.hir_id)
        .any(|(_, node)| is_test_named_module(node));

    if in_test_module {
        return true;
    }

    // Fallback: check if the source file looks like a test file
    let span = expr.span;
    if let Some(filename) = cx
        .tcx
        .sess
        .source_map()
        .span_to_filename(span)
        .into_local_path()
    {
        // Integration tests are in tests/ directory; use path components for
        // cross-platform compatibility (Windows uses backslashes)
        let has_tests_component = filename
            .components()
            .any(|c| matches!(c, Component::Normal(s) if s == OsStr::new("tests")));
        if has_tests_component {
            return true;
        }
    }

    false
}

fn is_test_named_module(node: hir::Node<'_>) -> bool {
    let hir::Node::Item(item) = node else {
        return false;
    };
    let hir::ItemKind::Mod { .. } = item.kind else {
        return false;
    };
    let Some(ident) = item.kind.ident() else {
        return false;
    };
    matches!(ident.name.as_str(), "test" | "tests")
}

fn extract_function_item(node: hir::Node<'_>) -> Option<&hir::Item<'_>> {
    let hir::Node::Item(item) = node else {
        return None;
    };
    matches!(item.kind, hir::ItemKind::Fn { .. }).then_some(item)
}

fn is_harness_marked_test_function(
    function_hir_id: hir::HirId,
    harness_marked_test_functions: &HashSet<hir::HirId>,
) -> bool {
    harness_marked_test_functions.contains(&function_hir_id)
}

fn collect_harness_marked_test_functions<'tcx>(cx: &LateContext<'tcx>) -> HashSet<hir::HirId> {
    let root_items = cx
        .tcx
        .hir_crate_items(())
        .free_items()
        .map(|id| cx.tcx.hir_item(id))
        .collect::<Vec<_>>();
    let mut harness_marked = HashSet::new();
    collect_harness_marked_test_functions_in_group(cx, root_items.as_slice(), &mut harness_marked);
    harness_marked
}

fn collect_harness_marked_test_functions_in_group<'tcx>(
    cx: &LateContext<'tcx>,
    items: &[&'tcx hir::Item<'tcx>],
    harness_marked: &mut HashSet<hir::HirId>,
) {
    for item in items
        .iter()
        .copied()
        .filter(|item| matches!(item.kind, hir::ItemKind::Fn { .. }))
    {
        let Some(function_ident) = item.kind.ident() else {
            continue;
        };

        let function_hir_id = item.hir_id();
        let function_name = function_ident.name;
        let function_span = item.span;
        if items.iter().copied().any(|sibling| {
            is_matching_harness_test_descriptor(
                function_hir_id,
                function_name,
                function_span,
                sibling,
            ) || has_companion_test_module(cx, function_hir_id, function_name, sibling)
        }) {
            harness_marked.insert(function_hir_id);
        }
    }

    for item in items {
        let hir::ItemKind::Mod(_, module) = item.kind else {
            continue;
        };

        let module_items = module
            .item_ids
            .iter()
            .map(|id| cx.tcx.hir_item(*id))
            .collect::<Vec<_>>();
        collect_harness_marked_test_functions_in_group(cx, module_items.as_slice(), harness_marked);
    }
}

fn is_matching_harness_test_descriptor(
    function_hir_id: hir::HirId,
    function_name: Symbol,
    function_span: Span,
    sibling: &hir::Item<'_>,
) -> bool {
    // `rustc --test` may synthesize a const descriptor that shares the test
    // function's name and source range. The wrapper function and descriptor can
    // carry different syntax contexts, so this must compare source bytes
    // rather than exact `Span` identity.
    sibling.hir_id() != function_hir_id
        && matches!(sibling.kind, hir::ItemKind::Const(..))
        && sibling.kind.ident().is_some_and(|ident| {
            ident.name == function_name && sibling.span.source_equal(function_span)
        })
}

fn has_companion_test_module<'tcx>(
    cx: &LateContext<'tcx>,
    function_hir_id: hir::HirId,
    function_name: Symbol,
    sibling: &'tcx hir::Item<'tcx>,
) -> bool {
    sibling.hir_id() != function_hir_id
        && sibling
            .kind
            .ident()
            .is_some_and(|ident| ident.name == function_name)
        && matches!(sibling.kind, hir::ItemKind::Mod(..))
        && module_has_harness_descriptor(cx, sibling)
}

fn module_has_harness_descriptor<'tcx>(
    cx: &LateContext<'tcx>,
    module_item: &'tcx hir::Item<'tcx>,
) -> bool {
    let hir::ItemKind::Mod(_, module) = module_item.kind else {
        return false;
    };

    module
        .item_ids
        .iter()
        .map(|item_id| cx.tcx.hir_item(*item_id))
        .any(|item| matches!(item.kind, hir::ItemKind::Const(..)))
}

// Check if any attribute is #[test].
fn has_test_attribute(attrs: &[hir::Attribute]) -> bool {
    has_test_like_hir_attributes(attrs, &[])
}

// Detect source-level test framework attributes.
//
// The `rustc --test` harness may consume the original built-in marker entirely
// and replace it with a sibling const descriptor. That recovery path is
// covered by the example-based regression in `lib_ui_tests.rs`; this helper
// still only inspects source-level HIR attributes.
#[cfg(test)]
fn is_test_attribute(attr: &hir::Attribute) -> bool {
    has_test_like_hir_attributes(std::slice::from_ref(attr), &[])
}

#[cfg(all(test, feature = "dylint-driver"))]
mod tests;
