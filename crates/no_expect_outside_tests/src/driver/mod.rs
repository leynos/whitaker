//! Lint crate forbidding `.expect(..)` outside test and doctest contexts.
//!
//! The lint inspects method calls named `expect`, verifies that the receiver
//! is an `Option` or `Result`, and checks the surrounding traversal context for
//! test-like attributes or `cfg(test)` guards. Doctest harnesses are skipped via
//! `Crate::is_doctest`, ensuring documentation examples remain ergonomic. When
//! no test context is present, the lint emits a denial with a note describing
//! the enclosing function and the receiver type to guide remediation. Teams can
//! extend the recognized test attributes through `dylint.toml` when bespoke
//! macros are in play.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;

use log::debug;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::{RemapPathScopeComponents, sym};
use serde::Deserialize;
use whitaker::SharedConfig;
use whitaker::hir::has_test_like_hir_attributes;
use whitaker_common::{AttributePath, Localizer, get_localizer_for_lint};

use crate::context::{collect_context, is_cfg_test_attribute, summarise_context};
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
            let mut marked = whitaker::hir::collect_harness_test_functions(cx);
            marked.extend(whitaker::hir::collect_rstest_companion_test_functions(cx));
            marked
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
        let (entries, has_test_context_ancestry) = collect_context(cx, expr.hir_id, additional);
        let summary = summarise_context(entries.as_slice(), has_test_context_ancestry, additional);

        if summary.is_test {
            return;
        }

        // Fallback: when compiled with --test (integration test crates), functions
        // with #[test] may not be detected via attributes if the test framework
        // processes them differently. Allow expect() in functions that appear to
        // be tests based on the harness context.
        if self.is_test_harness
            && is_likely_test_function(cx, expr, &self.harness_marked_test_functions, additional)
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
    let ty = cx
        .tcx
        .normalize_erasing_regions(cx.typing_env(), ty::Unnormalized::new_wip(ty))
        .peel_refs();

    let Some(adt) = ty.ty_adt_def() else {
        return false;
    };

    let def_id = adt.did();
    cx.tcx.is_diagnostic_item(sym::Option, def_id) || cx.tcx.is_diagnostic_item(sym::Result, def_id)
}

fn is_owner_test_function<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &hir::Expr<'tcx>,
    harness_marked_test_functions: &HashSet<hir::HirId>,
    additional_test_attributes: &[AttributePath],
) -> bool {
    let owner_hir_id: hir::HirId = expr.hir_id.owner.into();
    has_test_like_hir_attributes(cx.tcx.hir_attrs(owner_hir_id), additional_test_attributes)
        || is_harness_marked_test_function(owner_hir_id, harness_marked_test_functions)
}

fn ancestor_function_is_test<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &hir::Expr<'tcx>,
    harness_marked_test_functions: &HashSet<hir::HirId>,
    additional_test_attributes: &[AttributePath],
) -> bool {
    cx.tcx
        .hir_parent_iter(expr.hir_id)
        .filter_map(|(_, node)| extract_function_item(node))
        .any(|item| {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            has_test_like_hir_attributes(attrs, additional_test_attributes)
                || is_harness_marked_test_function(item.hir_id(), harness_marked_test_functions)
        })
}

fn is_in_tests_directory<'tcx>(cx: &LateContext<'tcx>) -> bool {
    cx.tcx.sess.local_crate_source_file().is_some_and(|source| {
        is_integration_test_crate_root(source.path(RemapPathScopeComponents::DIAGNOSTICS))
    })
}

fn is_integration_test_crate_root(crate_root: &Path) -> bool {
    let is_direct_test = crate_root
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|directory| directory == OsStr::new("tests"));
    let is_multi_file_test = crate_root.file_name() == Some(OsStr::new("main.rs"))
        && crate_root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .is_some_and(|directory| directory == OsStr::new("tests"));

    is_direct_test || is_multi_file_test
}
fn is_likely_test_function<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &hir::Expr<'tcx>,
    harness_marked_test_functions: &HashSet<hir::HirId>,
    additional_test_attributes: &[AttributePath],
) -> bool {
    is_owner_test_function(
        cx,
        expr,
        harness_marked_test_functions,
        additional_test_attributes,
    ) || ancestor_function_is_test(
        cx,
        expr,
        harness_marked_test_functions,
        additional_test_attributes,
    ) || is_in_cfg_test_module(cx, expr.hir_id)
        || is_in_tests_directory(cx)
}

fn is_in_cfg_test_module<'tcx>(cx: &LateContext<'tcx>, hir_id: hir::HirId) -> bool {
    cx.tcx.hir_parent_iter(hir_id).any(|(ancestor_id, node)| {
        let hir::Node::Item(item) = node else {
            return false;
        };
        if !matches!(item.kind, hir::ItemKind::Mod { .. }) {
            return false;
        }
        cx.tcx
            .hir_attrs(ancestor_id)
            .iter()
            .any(is_cfg_test_attribute)
    })
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
