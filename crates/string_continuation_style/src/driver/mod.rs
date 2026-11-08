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
mod edit_policy;
mod facts;
mod literal;
mod shadow;
mod span;
#[cfg(test)]
#[path = "../../tests/support/driver_tests.rs"]
mod tests;

pub(super) use facts::LiteralRewrite;

use self::{
    depth::{SafeSpans, safe_literal_spans},
    edit_policy::SourceEditPolicy,
    facts::{SourceContext, format_args_rewrite, plain_literal_rewrite},
    shadow::shadows_builtin_macros,
};

mod declaration {
    //! Dylint declaration and generated ABI registration glue.
    //!
    //! Keeping the `impl_early_lint!` expansion in its own module confines the
    //! items it generates — which have no source location that could carry
    //! documentation — so the enclosing `missing_docs` expectation covers only
    //! them and not the pass beside it.
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
    edit_policy: Option<SourceEditPolicy>,
    is_disabled_crate: bool,
}

impl Default for StringContinuationStyle {
    /// Starts the pass unconfigured, with no workspace policy resolved.
    ///
    /// `check_crate` replaces every field before the walk, so the defaults
    /// here only matter for the window between construction and that call.
    fn default() -> Self {
        Self {
            configuration: StringContinuationStyleConfig::default(),
            localizer: whitaker_common::Localizer::new(None),
            safe_spans: SafeSpans::default(),
            edit_policy: None,
            is_disabled_crate: false,
        }
    }
}

impl EarlyLintPass for StringContinuationStyle {
    /// Loads configuration and resolves the crate-wide facts before the walk.
    fn check_crate(&mut self, cx: &EarlyContext<'_>, krate: &ast::Crate) {
        self.configuration = load_configuration();
        self.localizer = load_localizer();
        let crate_name = crate_name(cx);
        // A crate that can substitute its own macro for one of the built-in
        // names makes every bare-name report unsound, so it is disabled
        // wholesale before the walk begins and no span is ever recorded.
        self.is_disabled_crate =
            self.configuration.is_excluded_crate(&crate_name) || shadows_builtin_macros(krate);
        self.safe_spans = if self.is_disabled_crate {
            SafeSpans::default()
        } else {
            let exclusions = self.configuration.path_exclusions();
            safe_literal_spans(krate, &crate_name, &exclusions)
        };
        // The workspace root and target directories are host facts, not
        // per-literal ones. Resolve them once here so the query that runs for
        // every candidate literal is a pure function of the path it is given.
        self.edit_policy = SourceEditPolicy::from_env();
    }

    /// Builds and emits a rewrite for one candidate expression, if any.
    fn check_expr(&mut self, cx: &EarlyContext<'_>, expression: &ast::Expr) {
        if self.is_disabled_crate {
            return;
        }
        let Some(edit_policy) = self.edit_policy.as_ref() else {
            return;
        };
        let context = SourceContext { cx, edit_policy };
        let rewrite = match &expression.kind {
            ast::ExprKind::Lit(_) => {
                plain_literal_rewrite(&context, expression, &self.safe_spans.plain_literals)
            }
            ast::ExprKind::FormatArgs(arguments) => format_args_rewrite(
                &context,
                expression,
                arguments,
                &self.safe_spans.format_strings,
            ),
            _ => None,
        };
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

/// Reads the crate name the compiler was invoked with.
///
/// Falls back to a placeholder rather than an empty string, so an exclusion
/// entry can never match it by accident.
fn crate_name(cx: &EarlyContext<'_>) -> String {
    cx.sess()
        .opts
        .crate_name
        .clone()
        .unwrap_or_else(|| String::from("unknown_crate"))
}
