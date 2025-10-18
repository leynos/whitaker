use crate::NO_EXPECT_OUTSIDE_TESTS;
use crate::context::ContextSummary;
use rustc_hir as hir;
use rustc_lint::{LateContext, LintContext};

pub(crate) fn emit_diagnostic(
    cx: &LateContext<'_>,
    expr: &hir::Expr<'_>,
    receiver: &hir::Expr<'_>,
    summary: &ContextSummary,
) {
    let receiver_ty = cx.typeck_results().expr_ty(receiver).peel_refs();
    let ty_display = receiver_ty.to_string();

    cx.span_lint(NO_EXPECT_OUTSIDE_TESTS, expr.span, |lint| {
        lint.primary_message("`.expect(..)` is not allowed outside tests or doctests");
        if let Some(name) = &summary.function_name {
            lint.note(format!(
                "Function `{name}` is not annotated with a recognised test attribute."
            ));
        } else {
            lint.note("No enclosing function was detected for the `.expect(..)` call.");
        }
        lint.note(format!("Receiver type: `{ty_display}`."));
        lint.help("Propagate the error or handle the `None`/`Err` case explicitly.");
    });
}
