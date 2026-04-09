//! Test context detection helpers for identifying test-only code.
//!
//! This module provides functions to determine whether code is inside a test
//! context by examining module names, harness descriptors, and companion
//! modules created by test framework macros like `rstest`.

use std::collections::HashSet;

use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};

use whitaker::hir::has_test_like_hir_attributes;

/// Recognize common test module naming conventions.
///
/// Matches exact names (`test`, `tests`) as well as modules whose name starts
/// with `test_` or `tests_`, or ends with `_test` or `_tests`. This covers
/// `#[path]`-loaded modules with non-standard names such as `service_tests`
/// that the test harness compiles under `--test`.
///
/// # Examples
///
/// ```text
/// "test"            → true
/// "tests"           → true
/// "test_helpers"    → true
/// "tests_util"      → true
/// "service_tests"   → true
/// "api_test"        → true
/// "my_service"      → false
/// "testing"         → false
/// "attest"          → false
/// ```
pub(super) fn has_test_module_name(name: &str) -> bool {
    matches!(name, "test" | "tests")
        || name.starts_with("test_")
        || name.starts_with("tests_")
        || name.ends_with("_test")
        || name.ends_with("_tests")
}

pub(super) fn is_test_named_module(node: hir::Node<'_>) -> bool {
    let hir::Node::Item(item) = node else {
        return false;
    };
    let hir::ItemKind::Mod(..) = item.kind else {
        return false;
    };
    let Some(ident) = item.kind.ident() else {
        return false;
    };
    has_test_module_name(ident.name.as_str())
}

pub(super) fn extract_function_item(node: hir::Node<'_>) -> Option<&hir::Item<'_>> {
    let hir::Node::Item(item) = node else {
        return None;
    };
    matches!(item.kind, hir::ItemKind::Fn { .. }).then_some(item)
}

pub(super) fn is_harness_marked_test_function(
    function_hir_id: hir::HirId,
    harness_marked_test_functions: &HashSet<hir::HirId>,
) -> bool {
    harness_marked_test_functions.contains(&function_hir_id)
}

pub(super) fn collect_harness_marked_test_functions<'tcx>(
    cx: &LateContext<'tcx>,
) -> HashSet<hir::HirId> {
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
            )
        }) {
            harness_marked.insert(function_hir_id);
        }

        // rstest with #[case] generates a companion module sharing the
        // function's name whose children are the actual test cases (with
        // harness const descriptors). Recognize the parent function as
        // test code when it has an rstest attribute AND such a companion
        // module exists. This avoids false positives from handwritten
        // functions paired with same-named test modules.
        let attrs = cx.tcx.hir_attrs(function_hir_id);
        if has_rstest_attribute(attrs) && has_companion_test_module(cx, function_name, items) {
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

/// rstest with #[case] expands into a bare function plus a companion module
/// of the same name containing harness const descriptors. Detect this pattern
/// so the parent function is treated as test-only code.
fn has_companion_test_module<'tcx>(
    cx: &LateContext<'tcx>,
    function_name: Symbol,
    siblings: &[&'tcx hir::Item<'tcx>],
) -> bool {
    siblings.iter().any(|sibling| {
        let hir::ItemKind::Mod(_, module) = sibling.kind else {
            return false;
        };
        let Some(mod_ident) = sibling.kind.ident() else {
            return false;
        };
        if mod_ident.name != function_name {
            return false;
        }
        // The module must contain test functions with matching harness const
        // descriptors to distinguish real rstest companion modules from
        // arbitrary same-named modules.
        let module_items = module
            .item_ids
            .iter()
            .map(|id| cx.tcx.hir_item(*id))
            .collect::<Vec<_>>();
        module_items.iter().any(|child| {
            if !matches!(child.kind, hir::ItemKind::Fn { .. }) {
                return false;
            }
            let Some(child_ident) = child.kind.ident() else {
                return false;
            };
            // Check if this function has a matching harness const descriptor
            module_items.iter().any(|sibling| {
                is_matching_harness_test_descriptor(
                    child.hir_id(),
                    child_ident.name,
                    child.span,
                    sibling,
                )
            })
        })
    })
}

// Check if any attribute is #[test].
pub(super) fn has_test_attribute(attrs: &[hir::Attribute]) -> bool {
    has_test_like_hir_attributes(attrs, &[])
}

// Check if any attribute is #[rstest] (or variants like #[rstest::rstest]).
fn has_rstest_attribute(attrs: &[hir::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let hir::Attribute::Unparsed(_) = attr else {
            return false;
        };
        let path_segments: Vec<String> = attr.path().into_iter().map(|s| s.to_string()).collect();
        // Match "rstest" or "rstest::rstest"
        matches!(
            path_segments
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice(),
            ["rstest"] | ["rstest", "rstest"]
        )
    })
}

// Detect source-level test framework attributes.
//
// The `rustc --test` harness may consume the original built-in marker entirely
// and replace it with a sibling const descriptor. That recovery path is
// covered by the example-based regression in `lib_ui_tests.rs`; this helper
// still only inspects source-level HIR attributes.
#[cfg(test)]
pub(super) fn is_test_attribute(attr: &hir::Attribute) -> bool {
    has_test_like_hir_attributes(std::slice::from_ref(attr), &[])
}
