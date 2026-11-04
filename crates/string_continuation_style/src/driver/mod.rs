//! Rustc-facing lint pass, wrapper-depth scan, and source-span recovery.
//!
//! The early pass selects only plain cooked literal expressions whose full
//! ancestor chain is source-authored, then delegates parsing and rewriting to
//! the pure string-literal crate.

use rustc_ast::ast;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};

use crate::{
    config::{StringContinuationStyleConfig, load_configuration},
    diagnostics::{emit_rewrite, load_localizer},
};

mod depth;
mod facts;
mod span;
#[cfg(test)]
#[path = "../../tests/support/driver_tests.rs"]
mod tests;

pub(super) use facts::LiteralRewrite;

use self::{
    depth::{SafeSpans, safe_literal_spans},
    facts::plain_literal_rewrite,
};

/// Dylint declaration and generated ABI registration glue.
mod declaration {
    #![expect(
        missing_docs,
        reason = "impl_early_lint! emits generated items without a documentable source location"
    )]

    use super::StringContinuationStyle;

    dylint_linting::impl_early_lint! {
        /// Flags source-line joins in cooked string literals.
        pub STRING_CONTINUATION_STYLE,
        Warn,
        "source-line joins in cooked strings should use concat! unless literal context requires a continuation escape",
        StringContinuationStyle::default()
    }
}

pub use declaration::STRING_CONTINUATION_STYLE;

/// Early lint pass for source-authored cooked string-literal joins.
pub struct StringContinuationStyle {
    configuration: StringContinuationStyleConfig,
    localizer: whitaker_common::Localizer,
    safe_spans: SafeSpans,
    is_disabled_crate: bool,
}

impl Default for StringContinuationStyle {
    fn default() -> Self {
        Self {
            configuration: StringContinuationStyleConfig::default(),
            localizer: whitaker_common::Localizer::new(None),
            safe_spans: SafeSpans::default(),
            is_disabled_crate: false,
        }
    }
}

impl EarlyLintPass for StringContinuationStyle {
    fn check_crate(&mut self, cx: &EarlyContext<'_>, krate: &ast::Crate) {
        self.configuration = load_configuration();
        self.localizer = load_localizer();
        let crate_name = crate_name(cx);
        self.is_disabled_crate = self.configuration.is_excluded_crate(&crate_name);
        self.safe_spans = if self.is_disabled_crate {
            SafeSpans::default()
        } else {
            let exclusions = self.configuration.path_exclusions();
            safe_literal_spans(krate, &crate_name, &exclusions)
        };
    }

    fn check_expr(&mut self, cx: &EarlyContext<'_>, expression: &ast::Expr) {
        if self.is_disabled_crate {
            return;
        }
        let ast::ExprKind::Lit(_) = &expression.kind else {
            return;
        };
        let rewrite = plain_literal_rewrite(cx, expression, &self.safe_spans.plain_literals);
        let Some(rewrite) = rewrite else {
            return;
        };
        emit_rewrite(
            cx,
            &self.localizer,
            self.configuration.applicability,
            rewrite,
        );
    }
}

fn crate_name(cx: &EarlyContext<'_>) -> String {
    cx.sess()
        .opts
        .crate_name
        .clone()
        .unwrap_or_else(|| String::from("unknown_crate"))
}
