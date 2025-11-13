//! Lint crate enforcing configurable maximum branches in conditional expressions.
//!
//! `conditional_max_n_branches` detects complex boolean predicates in `if`, `while`,
//! and match guard conditions that exceed the configurable `max_branches` threshold.
//! The lint encourages decomposition of complex conditionals into well-named helpers
//! or local variables to improve readability and maintainability.
use common::i18n::{Arguments, I18nError, Localizer, resolve_localizer};
use log::debug;
use rustc_hir as hir;
use rustc_hir::{BinOpKind, Expr, ExprKind, Guard, UnOp};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use whitaker::ConditionalMaxNBranchesConfig;

const LINT_NAME: &str = "conditional_max_n_branches";
const MESSAGE_KEY: &str = "conditional_max_n_branches";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConditionalDisposition {
    Accept,
    Reject,
}

dylint_linting::impl_late_lint! {
    pub CONDITIONAL_MAX_N_BRANCHES,
    Warn,
    "complex conditional in a branch; decompose or extract",
    ConditionalMaxNBranches::default()
}

/// Lint pass that tracks configuration and localisation state while checking conditionals.
pub struct ConditionalMaxNBranches {
    max_branches: usize,
    localizer: Localizer,
}

impl Default for ConditionalMaxNBranches {
    fn default() -> Self {
        Self {
            max_branches: ConditionalMaxNBranchesConfig::default().max_branches,
            localizer: Localizer::new(None),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for ConditionalMaxNBranches {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        self.max_branches = load_configuration();
        let environment_locale =
            std::env::var_os("DYLINT_LOCALE").and_then(|value| value.into_string().ok());
        let shared_config = whitaker::SharedConfig::load();
        let selection = resolve_localizer(None, environment_locale, shared_config.locale());

        selection.log_outcome(LINT_NAME);
        self.localizer = selection.into_localizer();
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let (cond_span, cond_expr) = match expr.kind {
            ExprKind::If(cond, ..) | ExprKind::While(cond, ..) => {
                // Exclude `if let` and `while let`
                if matches!(cond.kind, ExprKind::Let(..)) {
                    return;
                }
                (cond.span, cond)
            }
            _ => return,
        };

        let branch_count = count_predicate_atoms(cond_expr);
        debug!(
            target: LINT_NAME,
            "conditional expression has {branch_count} predicate atoms (limit: {limit})",
            limit = self.max_branches
        );

        let disposition = evaluate_conditional(branch_count, self.max_branches);
        if disposition != ConditionalDisposition::Reject {
            return;
        }

        let info = ConditionalDiagnosticInfo {
            span: cond_span,
            branch_count,
            limit: self.max_branches,
        };
        emit_diagnostic(cx, &info, &self.localizer);
    }

    fn check_match(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>, arms: &'tcx [hir::MatchArm<'tcx>]) {
        for arm in arms {
            if let Some(Guard::If(guard)) = arm.guard {
                let branch_count = count_predicate_atoms(guard);
                debug!(
                    target: LINT_NAME,
                    "match guard has {branch_count} predicate atoms (limit: {limit})",
                    limit = self.max_branches
                );

                let disposition = evaluate_conditional(branch_count, self.max_branches);
                if disposition != ConditionalDisposition::Reject {
                    continue;
                }

                let info = ConditionalDiagnosticInfo {
                    span: guard.span,
                    branch_count,
                    limit: self.max_branches,
                };
                emit_diagnostic(cx, &info, &self.localizer);
            }
        }
    }
}

fn evaluate_conditional(branch_count: usize, limit: usize) -> ConditionalDisposition {
    if branch_count > limit {
        ConditionalDisposition::Reject
    } else {
        ConditionalDisposition::Accept
    }
}

fn load_configuration() -> usize {
    match dylint_linting::config::<ConditionalMaxNBranchesConfig>(LINT_NAME) {
        Ok(Some(config)) => config.max_branches,
        Ok(None) => ConditionalMaxNBranchesConfig::default().max_branches,
        Err(error) => {
            debug!(
                target: LINT_NAME,
                "failed to parse `{}` configuration: {error}; using defaults",
                LINT_NAME
            );
            ConditionalMaxNBranchesConfig::default().max_branches
        }
    }
}

/// Counts the number of predicate atoms in a conditional expression.
/// 
/// A predicate atom is a boolean leaf (comparisons, boolean-returning calls,
/// boolean identifiers, etc.). Logical connectives (`&&`, `||`, `!`) form
/// the internal nodes of the predicate tree.
fn count_predicate_atoms(expr: &Expr<'_>) -> usize {
    match expr.kind {
        ExprKind::Binary(op, lhs, rhs)
            if matches!(op.node, BinOpKind::And | BinOpKind::Or) =>
        {
            count_predicate_atoms(lhs) + count_predicate_atoms(rhs)
        }
        ExprKind::Unary(UnOp::Not, inner) => count_predicate_atoms(inner),
        // Comparisons and other boolean operations count as single atoms
        ExprKind::Binary(op, ..)
            if matches!(
                op.node,
                BinOpKind::Eq | BinOpKind::Ne | BinOpKind::Lt | BinOpKind::Le | BinOpKind::Gt | BinOpKind::Ge
            ) =>
        {
            1
        }
        // Method calls, field accesses, and other expressions count as single atoms
        _ => 1,
    }
}

/// Diagnostic information for a conditional that exceeds branch limits.
struct ConditionalDiagnosticInfo {
    span: Span,
    branch_count: usize,
    limit: usize,
}

fn emit_diagnostic(cx: &LateContext<'_>, info: &ConditionalDiagnosticInfo, localizer: &Localizer) {
    use fluent_templates::fluent_bundle::FluentValue;
    use std::borrow::Cow;

    let mut args: Arguments<'_> = Arguments::default();
    args.insert(Cow::Borrowed("branches"), FluentValue::from(info.branch_count as i64));
    args.insert(Cow::Borrowed("limit"), FluentValue::from(info.limit as i64));

    let (primary, note, help) = localised_messages(localizer, &args).unwrap_or_else(|error| {
        debug!(
            target: LINT_NAME,
            "missing localisation for `{}`: {error}; using fallback strings",
            LINT_NAME
        );
        fallback_messages(info.branch_count, info.limit)
    });

    cx.span_lint(CONDITIONAL_MAX_N_BRANCHES, info.span, |lint| {
        lint.primary_message(primary);
        lint.note(note);
        lint.help(help);
    });
}

fn localised_messages(
    localizer: &Localizer,
    args: &Arguments<'_>,
) -> Result<(String, String, String), I18nError> {
    let primary = localizer.message_with_args(MESSAGE_KEY, args)?;
    let note = localizer.attribute_with_args(MESSAGE_KEY, "note", args)?;
    let help = localizer.attribute_with_args(MESSAGE_KEY, "help", args)?;

    Ok((primary, note, help))
}

fn fallback_messages(branch_count: usize, limit: usize) -> (String, String, String) {
    let primary = format!(
        "Conditional has {branch_count} predicate atoms which exceeds the {limit} branch limit."
    );
    let note = String::from("Complex conditionals hinder readability and contribute to the Complex Method smell.");
    let help = String::from("Extract the conditional to a well-named function or bind it to a local variable.");

    (primary, note, help)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(1, 2, ConditionalDisposition::Accept)]
    #[case(2, 2, ConditionalDisposition::Accept)]
    #[case(3, 2, ConditionalDisposition::Reject)]
    #[case(1, 1, ConditionalDisposition::Accept)]
    #[case(2, 1, ConditionalDisposition::Reject)]
    fn evaluate_conditional_behaviour(
        #[case] branch_count: usize,
        #[case] limit: usize,
        #[case] expected: ConditionalDisposition,
    ) {
        assert_eq!(evaluate_conditional(branch_count, limit), expected);
    }
}