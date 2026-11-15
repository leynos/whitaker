//! rustc-backed driver for the macro-aware cognitive complexity lint.
//!
//! The driver runs inside Dylint's late lint pass. For every function it
//! resolves a diagnostic anchor, selects the body that Clippy would score,
//! reduces that body to the ordered event stream [`crate::policy`] expects,
//! and emits a localized diagnostic when the total exceeds the configured
//! limit.

mod anchor;
mod config;
mod diagnostic;
mod traversal;

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use rustc_hir as hir;
use rustc_hir::{def_id::LocalDefId, intravisit::FnKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, sym};
use whitaker_common::{Localizer, get_localizer_for_lint, i18n::MessageKey};
use whitaker_lint_core::SharedConfig;

use self::config::{DEFAULT_MAX_COMPLEXITY, load_configuration};
use crate::policy;

const LINT_NAME: &str = "macro_aware_cognitive_complexity";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

/// Lint pass enforcing the authored cognitive complexity limit.
pub struct MacroAwareCognitiveComplexity {
    max_complexity: u64,
    localizer: Localizer,
    harness_test_functions: HashSet<hir::HirId>,
}

impl Default for MacroAwareCognitiveComplexity {
    fn default() -> Self {
        Self {
            max_complexity: DEFAULT_MAX_COMPLEXITY,
            localizer: Localizer::new(None),
            harness_test_functions: HashSet::new(),
        }
    }
}

mod registration {
    //! Provides the public Dylint registration for this lint.
    //!
    //! The `dylint_linting::impl_late_lint!` macro generates undocumented
    //! public registration items, so its expansion is scoped to this private
    //! module and only the documented lint static is re-exported.
    #![expect(
        missing_docs,
        reason = "dylint_linting macro expansion emits items with no documentable source location"
    )]

    use super::MacroAwareCognitiveComplexity;

    dylint_linting::impl_late_lint! {
        /// Lint flagging functions whose authored cognitive complexity exceeds
        /// the configured limit.
        pub MACRO_AWARE_COGNITIVE_COMPLEXITY,
        Warn,
        "functions should not exceed the configured authored cognitive complexity limit",
        MacroAwareCognitiveComplexity::default()
    }
}

pub use registration::MACRO_AWARE_COGNITIVE_COMPLEXITY;

impl MacroAwareCognitiveComplexity {
    /// Report whether a function is a `#[test]` function.
    ///
    /// Pinned Clippy skips `#[test]` functions but not doctests; this matches
    /// that choice. Two checks are needed because the marker moves during
    /// expansion. Outside a test build the function may still carry the
    /// attribute, and `TyCtxt::has_attr` is unavailable at this pin, so the
    /// resolved HIR attributes are inspected directly. In a `--test` build
    /// rustc has already consumed the attribute, so membership in the crate's
    /// harness-descriptor set is what identifies the test.
    fn is_test_function(&self, cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
        let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);
        self.harness_test_functions.contains(&hir_id)
            || cx
                .tcx
                .hir_attrs(hir_id)
                .iter()
                .any(|attribute| attribute.has_name(sym::test))
    }
}

impl<'tcx> LateLintPass<'tcx> for MacroAwareCognitiveComplexity {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.max_complexity = load_configuration().max_complexity;
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
        // A `--test` build consumes `#[test]` during expansion and moves the
        // marker onto a generated descriptor, so the authored function keeps
        // no attribute to inspect. Collecting the descriptors once per crate
        // keeps that lookup out of the per-function path.
        self.harness_test_functions = if cx.tcx.sess.opts.test {
            whitaker_lint_core::hir::collect_harness_test_functions(cx)
        } else {
            HashSet::new()
        };
    }

    // @codescene(disable:"Excess Number of Function Arguments") LateLintPass trait signature
    // (2026-11-12)
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _decl: &'tcx hir::FnDecl<'tcx>,
        body: &'tcx hir::Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        let anchor = anchor::resolve(cx, kind, span, def_id);

        // Both predicates are needed. `from_expansion` is rustc's own
        // predicate and rejects bodies that a macro generated; `is_generated`
        // classifies desugarings as authored, so it catches macro-written
        // source that rustc did not mark on the function span.
        if span.from_expansion() || whitaker_lint_core::hir::node_origin::is_generated(anchor.span)
        {
            return;
        }

        // A coroutine wrapper is a closure rustc synthesizes for an `async`
        // function or block. Unlike a macro-generated body it keeps an
        // authored span, so the predicates above do not reject it; scoring it
        // would double-count the body it wraps.
        if anchor::is_coroutine_wrapper(cx, kind, def_id) {
            return;
        }

        if self.is_test_function(cx, def_id) {
            return;
        }

        let Some(root) = traversal::body_root(cx, body, anchor::is_async(kind)) else {
            return;
        };

        let events = traversal::collect_events(root);
        let returns_result = traversal::body_returns_result(cx, root);
        let score = policy::score(&events, returns_result);

        if !score.exceeds(self.max_complexity) {
            return;
        }

        let report = diagnostic::Report {
            span: anchor.span,
            subject: &anchor.subject,
            score: score.total,
            limit: self.max_complexity,
        };
        diagnostic::emit(cx, &report, &self.localizer);
    }
}
