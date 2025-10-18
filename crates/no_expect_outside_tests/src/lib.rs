#![feature(rustc_private)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

//! Lint crate forbidding `.expect(..)` outside test and doctest contexts.
//!
//! The lint inspects method calls named `expect`, verifies that the receiver
//! is an `Option` or `Result`, and checks the surrounding traversal context for
//! test-like attributes or `cfg(test)` guards. When no test context is present,
//! the lint emits a denial with a note describing the enclosing function and the
//! receiver type to guide remediation.

use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::Ty;
use rustc_span::sym;

mod context;
mod diagnostics;

use context::{collect_context, summarise_context};
use diagnostics::emit_diagnostic;

dylint_linting::impl_late_lint! {
    pub NO_EXPECT_OUTSIDE_TESTS,
    Deny,
    "`.expect(..)` must not be used outside of test or doctest contexts",
    NoExpectOutsideTests::default()
}

/// Lint pass that tracks contexts while checking method calls.
#[derive(Default)]
pub struct NoExpectOutsideTests;

impl<'tcx> LateLintPass<'tcx> for NoExpectOutsideTests {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        let hir::ExprKind::MethodCall(segment, receiver, ..) = expr.kind else {
            return;
        };

        if segment.ident.name != sym::expect {
            return;
        }

        if !receiver_is_option_or_result(cx, receiver) {
            return;
        }

        let (entries, has_cfg_test) = collect_context(cx, expr.hir_id);
        let summary = summarise_context(entries.as_slice(), has_cfg_test);

        if summary.is_test {
            return;
        }

        emit_diagnostic(cx, expr, receiver, &summary);
    }
}

fn receiver_is_option_or_result<'tcx>(
    cx: &LateContext<'tcx>,
    receiver: &'tcx hir::Expr<'tcx>,
) -> bool {
    let ty = cx.typeck_results().expr_ty(receiver).peel_refs();

    ty_is_option_or_result(cx, ty)
}

fn ty_is_option_or_result<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    let Some(adt) = ty.ty_adt_def() else {
        return false;
    };

    let def_id = adt.did();
    cx.tcx.is_diagnostic_item(sym::Option, def_id) || cx.tcx.is_diagnostic_item(sym::Result, def_id)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod behaviour;

#[cfg(test)]
mod ui {
    whitaker::declare_ui_tests!("ui");
}
