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

use std::ffi::OsStr;
use std::path::Component;

use common::{AttributePath, Localizer, get_localizer_for_lint};
use log::debug;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::sym;
use serde::Deserialize;
use whitaker::SharedConfig;

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
    localizer: Localizer,
}

impl Default for NoExpectOutsideTests {
    fn default() -> Self {
        Self {
            is_doctest: false,
            is_test_harness: false,
            additional_test_attributes: Vec::new(),
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
        if self.is_test_harness && is_likely_test_function(cx, expr) {
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
fn is_likely_test_function<'tcx>(cx: &LateContext<'tcx>, expr: &hir::Expr<'tcx>) -> bool {
    // First, check if any enclosing function has a test attribute
    let has_test_attr = cx
        .tcx
        .hir_parent_iter(expr.hir_id)
        .filter_map(|(_, node)| extract_function_item(node))
        .any(|item| {
            let attrs = cx.tcx.hir_attrs(item.hir_id());
            has_test_attribute(attrs)
        });

    if has_test_attr {
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

// Check if any attribute is #[test].
fn has_test_attribute(attrs: &[hir::Attribute]) -> bool {
    attrs.iter().any(is_test_attribute)
}

// Detect test framework attributes.
//
// Test attributes (#[test], #[rstest], #[tokio::test], etc.) are represented as
// Unparsed HIR attributes. The Parsed variant is reserved for compiler-internal
// attributes like #[must_use] and #[doc], not for test framework annotations.
// This function therefore only inspects Unparsed attributes.
fn is_test_attribute(attr: &hir::Attribute) -> bool {
    let hir::Attribute::Unparsed(_) = attr else {
        return false;
    };

    let path = attr.path();

    // Check for built-in #[test] attribute via symbol comparison (fast path)
    if path.len() == 1 && path[0] == sym::test {
        return true;
    }

    // Match against known test attribute patterns (must match full paths to
    // avoid false positives like #[tokio::main] or #[rstest::fixture]).
    // Use direct length and element checks to avoid per-attribute allocation.
    match path.len() {
        1 => matches!(path[0].as_str(), "rstest" | "case"),
        2 => {
            let first = path[0].as_str();
            let second = path[1].as_str();
            matches!(
                (first, second),
                ("rstest", "rstest")
                    | ("rstest", "case")
                    | ("tokio", "test")
                    | ("async_std", "test")
                    | ("gpui", "test")
            )
        }
        _ => false,
    }
}

#[cfg(all(test, feature = "dylint-driver"))]
mod tests {
    use super::*;
    use rstest::rstest;
    use rustc_ast::AttrStyle;
    use rustc_hir::attrs::AttributeKind as HirAttributeKind;
    use rustc_span::symbol::Ident;
    use rustc_span::{AttrId, DUMMY_SP, create_default_session_globals_then};

    // -------------------------------------------------------------------------
    // Test fixtures for HIR attributes
    // -------------------------------------------------------------------------

    fn hir_attribute_from_segments(segments: &[&str]) -> hir::Attribute {
        let path_segments = segments
            .iter()
            .map(|segment| Ident::from_str(segment))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let attr_item = hir::AttrItem {
            path: hir::AttrPath {
                segments: path_segments,
                span: DUMMY_SP,
            },
            args: hir::AttrArgs::Empty,
            id: hir::HashIgnoredAttrId {
                attr_id: AttrId::from_u32(0),
            },
            style: AttrStyle::Outer,
            span: DUMMY_SP,
        };

        hir::Attribute::Unparsed(Box::new(attr_item))
    }

    fn parsed_must_use_attribute() -> hir::Attribute {
        hir::Attribute::Parsed(HirAttributeKind::MustUse {
            span: DUMMY_SP,
            reason: None,
        })
    }

    // -------------------------------------------------------------------------
    // Tests for is_test_attribute
    // -------------------------------------------------------------------------

    #[rstest]
    #[case::builtin_test(&["test"])]
    #[case::rstest_single(&["rstest"])]
    #[case::case_single(&["case"])]
    #[case::tokio_test(&["tokio", "test"])]
    #[case::async_std_test(&["async_std", "test"])]
    #[case::gpui_test(&["gpui", "test"])]
    #[case::rstest_rstest(&["rstest", "rstest"])]
    #[case::rstest_case(&["rstest", "case"])]
    fn is_test_attribute_accepts_test_patterns(#[case] segments: &[&str]) {
        create_default_session_globals_then(|| {
            let attr = hir_attribute_from_segments(segments);
            assert!(is_test_attribute(&attr));
        });
    }

    #[rstest]
    #[case::tokio_main(&["tokio", "main"])]
    #[case::rstest_fixture(&["rstest", "fixture"])]
    #[case::inline(&["inline"])]
    #[case::derive(&["derive"])]
    #[case::allow(&["allow"])]
    #[case::cfg(&["cfg"])]
    #[case::three_segments(&["foo", "bar", "test"])]
    fn is_test_attribute_rejects_non_test_attributes(#[case] segments: &[&str]) {
        create_default_session_globals_then(|| {
            let attr = hir_attribute_from_segments(segments);
            assert!(!is_test_attribute(&attr));
        });
    }

    #[test]
    fn is_test_attribute_returns_false_for_parsed_attributes() {
        let attr = parsed_must_use_attribute();
        assert!(!is_test_attribute(&attr));
    }

    // -------------------------------------------------------------------------
    // Tests for has_test_attribute
    // -------------------------------------------------------------------------

    #[test]
    fn has_test_attribute_returns_true_when_test_present() {
        create_default_session_globals_then(|| {
            let attrs = [
                hir_attribute_from_segments(&["inline"]),
                hir_attribute_from_segments(&["test"]),
            ];
            assert!(has_test_attribute(&attrs));
        });
    }

    #[test]
    fn has_test_attribute_returns_true_for_rstest() {
        create_default_session_globals_then(|| {
            let attrs = [hir_attribute_from_segments(&["rstest"])];
            assert!(has_test_attribute(&attrs));
        });
    }

    #[test]
    fn has_test_attribute_returns_true_for_tokio_test() {
        create_default_session_globals_then(|| {
            let attrs = [hir_attribute_from_segments(&["tokio", "test"])];
            assert!(has_test_attribute(&attrs));
        });
    }

    #[test]
    fn has_test_attribute_returns_false_for_empty_array() {
        let attrs: [hir::Attribute; 0] = [];
        assert!(!has_test_attribute(&attrs));
    }

    #[test]
    fn has_test_attribute_returns_false_for_non_test_attributes() {
        create_default_session_globals_then(|| {
            let attrs = [
                hir_attribute_from_segments(&["inline"]),
                hir_attribute_from_segments(&["derive"]),
                hir_attribute_from_segments(&["tokio", "main"]),
            ];
            assert!(!has_test_attribute(&attrs));
        });
    }

    #[test]
    fn has_test_attribute_handles_parsed_attributes() {
        let attrs = [parsed_must_use_attribute()];
        assert!(!has_test_attribute(&attrs));
    }

    // -------------------------------------------------------------------------
    // Tests for is_test_named_module and extract_function_item
    //
    // These functions operate on hir::Node and hir::Item types which require
    // complex HIR context to construct (owner IDs, def IDs, etc.). They are
    // part of the `is_likely_test_function` fallback which only activates when
    // rustc runs with the `--test` flag (integration test crates). The pattern
    // matching logic is straightforward:
    //
    // - is_test_named_module: checks if a module is named "test" or "tests"
    // - extract_function_item: extracts function items from HIR nodes
    //
    // Full unit testing would require mocking the entire HIR infrastructure,
    // which provides diminishing returns given the simplicity of the logic.
    // The fallback behaviour is validated by running integration tests that
    // use `.expect()` in test crates compiled with `--test`.
    // -------------------------------------------------------------------------
}
