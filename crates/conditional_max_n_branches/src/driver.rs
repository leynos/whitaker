//! Detect conditionals that exceed the configured number of predicate branches.
//!
//! The lint counts boolean branches within `if`, `while`, and `match` guard
//! predicates, flagging expressions that involve more than the configured
//! number of short-circuit branches. Diagnostics are localised through the
//! shared Fluent bundles so helper text stays consistent with other lints.

use std::borrow::Cow;

use common::i18n::{DiagnosticMessageSet, MessageKey};
use common::{
    Arguments, FALLBACK_LOCALE, Localizer, MessageResolution, branch_phrase,
    get_localizer_for_lint, safe_resolve_message_set,
};
use fluent_templates::fluent_bundle::FluentValue;
use log::debug;
use rustc_hir as hir;
use rustc_hir::{BinOpKind, ExprKind, LoopSource, UnOp};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{DesugaringKind, Span};
use serde::Deserialize;
use whitaker::SharedConfig;

const LINT_NAME: &str = "conditional_max_n_branches";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Config {
    #[serde(default = "Config::default_max_branches")]
    max_branches: usize,
}

impl Config {
    const fn default_max_branches() -> usize {
        2
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_branches: Self::default_max_branches(),
        }
    }
}

/// Lint pass enforcing predicate branch limits.
pub struct ConditionalMaxNBranches {
    max_branches: usize,
    localizer: Localizer,
}

impl Default for ConditionalMaxNBranches {
    fn default() -> Self {
        Self {
            max_branches: Config::default().max_branches,
            localizer: Localizer::new(None),
        }
    }
}

dylint_linting::impl_late_lint! {
    pub CONDITIONAL_MAX_N_BRANCHES,
    Warn,
    "complex conditionals should be decomposed when they exceed the configured branch limit",
    ConditionalMaxNBranches::default()
}

impl<'tcx> LateLintPass<'tcx> for ConditionalMaxNBranches {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        self.max_branches = load_configuration().max_branches.max(1);
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        match expr.kind {
            ExprKind::If(cond, ..) => {
                if expr.span.desugaring_kind() == Some(DesugaringKind::WhileLoop) {
                    return;
                }
                self.inspect_condition(cx, ConditionKind::If, cond);
            }
            ExprKind::Loop(block, _, LoopSource::While, _) => {
                if let Some(cond) = extract_while_condition(block) {
                    self.inspect_condition(cx, ConditionKind::While, cond);
                }
            }
            ExprKind::Match(_, arms, _) => self.inspect_match_guards(cx, arms),
            _ => {}
        }
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

impl ConditionalMaxNBranches {
    fn inspect_condition(&self, cx: &LateContext<'_>, kind: ConditionKind, expr: &hir::Expr<'_>) {
        if matches!(expr.kind, ExprKind::Let(..)) {
            return;
        }

        let branches = count_branches(expr);
        if evaluate_condition(branches, self.max_branches) == ConditionDisposition::WithinLimit {
            return;
        }

        let metadata = ConditionMetadata {
            kind,
            span: expr.span,
            branches,
        };
        emit_diagnostic(cx, &metadata, self.max_branches, &self.localizer);
    }

    fn inspect_match_guards(&self, cx: &LateContext<'_>, arms: &[hir::Arm<'_>]) {
        for arm in arms {
            if let Some(expr) = arm.guard {
                self.inspect_condition(cx, ConditionKind::MatchGuard, expr);
            }
        }
    }
}

fn extract_while_condition<'hir>(block: &'hir hir::Block<'hir>) -> Option<&'hir hir::Expr<'hir>> {
    let expr = block.expr?;
    if let ExprKind::If(cond, ..) = expr.kind {
        Some(cond)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConditionDisposition {
    WithinLimit,
    ExceedsLimit,
}

const fn evaluate_condition(branches: usize, limit: usize) -> ConditionDisposition {
    if branches > limit {
        ConditionDisposition::ExceedsLimit
    } else {
        ConditionDisposition::WithinLimit
    }
}

#[derive(Clone, Copy, Debug)]
struct ConditionMetadata {
    kind: ConditionKind,
    span: Span,
    branches: usize,
}

#[derive(Clone, Copy, Debug)]
enum ConditionKind {
    If,
    While,
    MatchGuard,
}

impl ConditionKind {
    const fn display_name(self) -> &'static str {
        match self {
            Self::If => "if condition",
            Self::While => "while condition",
            Self::MatchGuard => "match guard",
        }
    }
}

fn count_branches(expr: &hir::Expr<'_>) -> usize {
    match expr.kind {
        ExprKind::Binary(op, lhs, rhs) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
            count_branches(lhs) + count_branches(rhs)
        }
        ExprKind::Unary(UnOp::Not, inner) => count_branches(inner),
        ExprKind::DropTemps(inner) => count_branches(inner),
        ExprKind::Block(block, _) => match block.expr {
            Some(inner) => count_branches(inner),
            None => 1,
        },
        ExprKind::If(cond, ..) => count_branches(cond),
        _ => 1,
    }
}

fn emit_diagnostic(
    cx: &LateContext<'_>,
    metadata: &ConditionMetadata,
    limit: usize,
    localizer: &Localizer,
) {
    let mut args: Arguments<'static> = Arguments::default();
    args.insert(
        Cow::Borrowed("name"),
        FluentValue::from(metadata.kind.display_name()),
    );
    args.insert(
        Cow::Borrowed("branches"),
        FluentValue::from(metadata.branches as i64),
    );
    args.insert(Cow::Borrowed("limit"), FluentValue::from(limit as i64));
    let branch_phrase_text = branch_phrase(localizer.locale(), metadata.branches);
    args.insert(
        Cow::Borrowed("branch_phrase"),
        FluentValue::String(Cow::Owned(branch_phrase_text)),
    );
    let limit_phrase_text = branch_phrase(localizer.locale(), limit);
    args.insert(
        Cow::Borrowed("limit_phrase"),
        FluentValue::String(Cow::Owned(limit_phrase_text)),
    );

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };
    let messages = safe_resolve_message_set(
        localizer,
        resolution,
        |message| {
            cx.tcx.sess.dcx().span_delayed_bug(metadata.span, message);
        },
        || fallback_messages(metadata.kind, metadata.branches, limit),
    );

    let primary = normalise_isolation_marks(messages.primary());
    let note = normalise_isolation_marks(messages.note());
    let help = normalise_isolation_marks(messages.help());

    cx.span_lint(CONDITIONAL_MAX_N_BRANCHES, metadata.span, move |lint| {
        lint.primary_message(primary);
        lint.span_note(metadata.span, note);
        lint.help(help);
    });
}

fn normalise_isolation_marks(text: &str) -> String {
    if text
        .chars()
        .any(|character| matches!(character, '\u{2068}' | '\u{2069}' | '\u{FFFD}'))
    {
        text.chars()
            .map(|character| match character {
                '\u{2068}' | '\u{2069}' | '\u{FFFD}' => '"',
                other => other,
            })
            .collect()
    } else {
        text.to_string()
    }
}

fn fallback_messages(kind: ConditionKind, branches: usize, limit: usize) -> DiagnosticMessageSet {
    let branch_phrase_text = branch_phrase(FALLBACK_LOCALE, branches);
    let limit_phrase_text = branch_phrase(FALLBACK_LOCALE, limit);
    let primary = format!(
        "Collapse the {} to {} or fewer.",
        kind.display_name(),
        limit_phrase_text
    );
    let note = format!(
        "The {} currently contains {branch_phrase_text}.",
        kind.display_name()
    );
    let help = format!(
        "Extract helper functions or simplify the {} to reduce branching.",
        kind.display_name()
    );

    DiagnosticMessageSet::new(primary, note, help)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(1, 2, ConditionDisposition::WithinLimit)]
    #[case(2, 2, ConditionDisposition::WithinLimit)]
    #[case(3, 2, ConditionDisposition::ExceedsLimit)]
    fn evaluate_condition_respects_limit(
        #[case] branches: usize,
        #[case] limit: usize,
        #[case] expected: ConditionDisposition,
    ) {
        assert_eq!(evaluate_condition(branches, limit), expected);
    }
}

#[cfg(test)]
#[path = "tests/behaviour.rs"]
mod behaviour;
