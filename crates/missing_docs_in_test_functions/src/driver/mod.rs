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
use rustc_lint::{EarlyContext, EarlyLintPass};
use rustc_span::Span;

use crate::policy::{CandidateFacts, ContainerStack, FunctionShape, Verdict, select};

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

    fn check_item_post(&mut self, _cx: &EarlyContext<'_>, _item: &Item) { self.leave(); }

    fn check_impl_item(&mut self, cx: &EarlyContext<'_>, item: &AssocItem) {
        self.check_associated_item(cx, item);
    }

    fn check_impl_item_post(&mut self, _cx: &EarlyContext<'_>, _item: &AssocItem) { self.leave(); }

    fn check_trait_item(&mut self, cx: &EarlyContext<'_>, item: &AssocItem) {
        self.check_associated_item(cx, item);
    }

    fn check_trait_item_post(&mut self, _cx: &EarlyContext<'_>, _item: &AssocItem) { self.leave(); }
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
            let facts = facts::item_facts(cx, item, function, context);
            report(cx, &function.ident, item.span, facts);
        }
        self.containers
            .enter(container::associated_item_container_kind(&item.kind));
    }

    /// Pops the frame the matching pre-hook pushed.
    ///
    /// This pops without verifying that the frame matches the item being left.
    /// Rustc calls each pre/post pair unconditionally around the walk of that
    /// node, so the pairing is balanced by construction and there is nothing to
    /// compare against: a check would only restate the invariant. The crate has
    /// no way to report a mismatch in any case, because a Dylint lint is a
    /// `cdylib` loaded under `RTLD_LOCAL` and its `log` statics are private.
    ///
    /// Three things hold the invariant: the stack's own unit tests, the
    /// tree-oracle property test, and the fixtures whose blessed diagnostics
    /// differ for each enclosing container kind, so a pre/post pair that pushed
    /// and popped different frames would change them.
    fn leave(&mut self) { let _popped = self.containers.exit(); }
}

/// Emits the diagnostic when the candidate diagnoses.
fn report(cx: &EarlyContext<'_>, ident: &rustc_span::Ident, item: Span, facts: CandidateFacts) {
    if select(facts) == Verdict::Diagnose {
        diagnostics::emit_undocumented(
            cx,
            diagnostics::diagnostic_span(ident.span, item),
            ident.name.as_str(),
        );
    }
}
