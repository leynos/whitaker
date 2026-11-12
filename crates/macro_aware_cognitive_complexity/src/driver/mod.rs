//! rustc-backed driver for the macro-aware cognitive complexity lint.
//!
//! The driver runs inside Dylint's late lint pass. For every function it
//! resolves a diagnostic anchor, selects the body that Clippy would score,
//! reduces that body to the ordered event stream [`crate::policy`] expects,
//! and emits a localized diagnostic when the total exceeds the configured
//! limit.

mod anchor;
mod diagnostic;
mod traversal;

#[cfg(test)]
mod tests;

use log::debug;
use rustc_hir as hir;
use rustc_hir::{def_id::LocalDefId, intravisit::FnKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, sym};
use serde::Deserialize;
use whitaker_common::{Localizer, get_localizer_for_lint, i18n::MessageKey};
use whitaker_lint_core::SharedConfig;

use crate::policy;

const LINT_NAME: &str = "macro_aware_cognitive_complexity";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);
const DEFAULT_MAX_COMPLEXITY: u64 = 25;

/// Parsed `dylint.toml` configuration for this lint.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Config {
    max_complexity: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_complexity: DEFAULT_MAX_COMPLEXITY,
        }
    }
}

/// Lint pass enforcing the authored cognitive complexity limit.
pub struct MacroAwareCognitiveComplexity {
    max_complexity: u64,
    localizer: Localizer,
}

impl Default for MacroAwareCognitiveComplexity {
    fn default() -> Self {
        Self {
            max_complexity: DEFAULT_MAX_COMPLEXITY,
            localizer: Localizer::new(None),
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

impl<'tcx> LateLintPass<'tcx> for MacroAwareCognitiveComplexity {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        self.max_complexity = load_configuration().max_complexity;
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

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

        if is_test_function(cx, def_id) {
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

/// Load the lint configuration, falling back to defaults on absence or error.
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

/// Report whether a function carries the built-in `#[test]` attribute.
///
/// Pinned Clippy skips `#[test]` functions but not doctests; this matches that
/// choice. `TyCtxt::has_attr` is unavailable at this pin, so the resolved HIR
/// attributes are inspected directly.
fn is_test_function(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);
    cx.tcx
        .hir_attrs(hir_id)
        .iter()
        .any(|attribute| attribute.has_name(sym::test))
}
