//! Rustc-facing pre-expansion lint pass.
//!
//! The pass maps one AST item onto [`CandidateFacts`], asks the pure policy for
//! a verdict, and emits a diagnostic at the authored identifier. It records
//! where it is in the item tree with a balanced container stack, so a
//! function's context comes from the frames enclosing it rather than from a
//! span comparison.
//!
//! Rustc runs a pre-expansion pass once for the crate root and once more for
//! every out-of-line module file as expansion loads it. Module-file runs walk
//! only that module's items and never call `check_crate`, so this pass keeps no
//! per-crate state: each hook is self-contained, and the localizer is cached
//! process-wide in [`diagnostics`].

use rustc_ast::ast::{AssocItem, AssocItemKind, Item, ItemKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::policy::{
    CandidateFacts,
    ContainerKind,
    ContainerStack,
    FunctionShape,
    Verdict,
    select,
};

mod attributes;
mod container;
mod diagnostics;
mod facts;

#[cfg(all(test, feature = "dylint-driver"))]
#[path = "tests.rs"]
mod tests;

mod declaration {
    //! Dylint declaration and generated ABI registration glue.
    //!
    //! Keeping the `impl_pre_expansion_lint!` expansion in its own module
    //! confines the items it generates — which have no source location that
    //! could carry documentation — so the enclosing `missing_docs` expectation
    //! covers only them and not the pass beside it.
    #![expect(
        missing_docs,
        reason = "impl_pre_expansion_lint! emits generated items without a documentable source \
                  location"
    )]

    use super::MissingDocsInTestFunctions;

    dylint_linting::impl_pre_expansion_lint! {
        /// Flags undocumented functions compiled for a test harness.
        pub MISSING_DOCS_IN_TEST_FUNCTIONS,
        Warn,
        "test functions should carry a documentation comment",
        MissingDocsInTestFunctions::default()
    }
}

pub use declaration::MISSING_DOCS_IN_TEST_FUNCTIONS;

/// Canonical lint name used for `dylint.toml` lookups and log targets.
pub(crate) const LINT_NAME: &str = "missing_docs_in_test_functions";

/// Pre-expansion lint pass for undocumented test functions.
#[derive(Default)]
pub struct MissingDocsInTestFunctions {
    containers: ContainerStack,
}

impl EarlyLintPass for MissingDocsInTestFunctions {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if let ItemKind::Fn(function) = &item.kind {
            let context = self.containers.classify(FunctionShape::Free);
            let facts = facts::item_facts(cx, item, function, context);
            report(cx, &function.ident, item.span, facts);
        }
        self.containers
            .enter(container::item_container_kind(&item.kind));
    }

    fn check_item_post(&mut self, _cx: &EarlyContext<'_>, item: &Item) {
        self.leave(container::item_container_kind(&item.kind));
    }

    fn check_impl_item(&mut self, cx: &EarlyContext<'_>, item: &AssocItem) {
        self.check_associated_item(cx, item);
    }

    fn check_impl_item_post(&mut self, _cx: &EarlyContext<'_>, item: &AssocItem) {
        self.leave(container::associated_item_container_kind(&item.kind));
    }

    fn check_trait_item(&mut self, cx: &EarlyContext<'_>, item: &AssocItem) {
        self.check_associated_item(cx, item);
    }

    fn check_trait_item_post(&mut self, _cx: &EarlyContext<'_>, item: &AssocItem) {
        self.leave(container::associated_item_container_kind(&item.kind));
    }
}

impl MissingDocsInTestFunctions {
    /// Classifies and reports one associated function, then pushes its frame.
    ///
    /// Trait and impl items share this because the classification already reads
    /// the enclosing container's kind from the stack; the hook that fires says
    /// which syntax introduced the item, and the stack says what encloses it.
    fn check_associated_item(&mut self, cx: &EarlyContext<'_>, item: &AssocItem) {
        if let AssocItemKind::Fn(function) = &item.kind {
            let context = self.containers.classify(FunctionShape::Associated);
            let facts = facts::associated_item_facts(cx, item, function, context);
            report(cx, &function.ident, item.span, facts);
        }
        self.containers
            .enter(container::associated_item_container_kind(&item.kind));
    }

    /// Pops the frame the matching pre-hook pushed, logging any imbalance.
    ///
    /// A mismatch means this pass lost track of the item tree. It is logged
    /// rather than asserted because a lint must not abort the user's build.
    fn leave(&mut self, expected: ContainerKind) {
        let popped = self.containers.exit();
        if popped != Some(expected) {
            log::debug!(
                target: LINT_NAME,
                "unbalanced container stack: expected {expected:?}, popped {popped:?}"
            );
        }
    }
}

/// Logs one candidate's verdict and emits the diagnostic when it diagnoses.
///
/// Every classified function produces a debug event, so an inert run — one
/// where no diagnostic appears because the compilation is not a test harness —
/// is observable with `RUST_LOG=missing_docs_in_test_functions=debug`. The
/// event names the function's file and carries the skip reason, which is what
/// distinguishes "the gate rejected this" from "the pass never ran". The path
/// is computed only when debug logging is enabled.
fn report(cx: &EarlyContext<'_>, ident: &rustc_span::Ident, item: Span, facts: CandidateFacts) {
    let verdict = select(facts);
    // Guarded rather than passed inline: a `log::debug!` argument is evaluated
    // whether or not the level is enabled, and the label costs a source-map
    // lookup per function.
    if log::log_enabled!(target: LINT_NAME, log::Level::Debug) {
        log::debug!(
            target: LINT_NAME,
            "classified `{}` in {} as {verdict:?}",
            ident.name,
            facts::source_label(cx.sess(), item),
        );
    }
    if verdict == Verdict::Diagnose {
        diagnostics::emit_undocumented(
            cx,
            diagnostics::diagnostic_span(ident.span, item),
            ident.name.as_str(),
        );
    }
}
