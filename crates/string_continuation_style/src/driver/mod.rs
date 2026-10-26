//! Rustc-facing pass state and traversal for the string continuation lint.

use rustc_lint::EarlyLintPass;

/// Early lint pass for source-authored string-literal continuations.
#[derive(Default)]
pub struct StringContinuationStyle;

mod lint_pass {
    //! Generated `LintPass` glue for the scaffold's empty lint list.
    #![expect(
        missing_docs,
        reason = "rustc_session::impl_lint_pass! emits an undocumented accessor"
    )]

    use super::StringContinuationStyle;

    rustc_session::impl_lint_pass!(StringContinuationStyle => []);
}

impl EarlyLintPass for StringContinuationStyle {}
