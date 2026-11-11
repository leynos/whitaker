//! Behaviour-driven coverage for module documentation detection.
//!
//! The scenarios describe what the HIR adapter collects from a module body and
//! what the classifier concludes from those records. Candidates carry an
//! `is_inner` flag so the world applies the same body containment rule the
//! adapter applies to real spans.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use rustc_span::{BytePos, DUMMY_SP, Span};

use super::{
    ModuleDocDisposition,
    hir_attrs::{ModuleAttr, classify},
};

/// One attribute offered to the adapter, plus whether it lies in the body.
#[derive(Clone, Copy)]
struct Candidate {
    attr: ModuleAttr,
    is_inner: bool,
}

#[derive(Default)]
struct ModuleWorld {
    candidates: RefCell<Vec<Candidate>>,
    result: RefCell<Option<ModuleDocDisposition>>,
}

impl ModuleWorld {
    /// Records a doc attribute written inside the module body.
    fn push_inner_doc(&self, lo: u32) { self.push(lo, true, true); }

    /// Records a non-doc attribute written inside the module body.
    fn push_inner_other(&self, lo: u32) { self.push(lo, false, true); }

    /// Records a doc attribute written outside the module body.
    fn push_outer_doc(&self, lo: u32) { self.push(lo, true, false); }

    /// Records one candidate at a byte offset, keeping source order explicit.
    fn push(&self, lo: u32, is_doc: bool, is_inner: bool) {
        self.candidates.borrow_mut().push(Candidate {
            attr: ModuleAttr {
                is_doc,
                span: span_at(lo),
            },
            is_inner,
        });
    }

    /// Classifies only the candidates the adapter would retain.
    fn evaluate(&self) {
        let candidates = self.candidates.borrow();
        let attrs: Vec<ModuleAttr> = candidates
            .iter()
            .filter(|candidate| candidate.is_inner)
            .map(|candidate| candidate.attr)
            .collect();
        self.result.replace(Some(classify(&attrs, true)));
    }

    /// Returns the recorded classification outcome.
    fn outcome(&self) -> ModuleDocDisposition {
        *self
            .result
            .borrow()
            .as_ref()
            .expect("detector outcome should be recorded")
    }
}

/// Builds a span at a distinct byte offset so record order is unambiguous.
///
/// `Span::new` swaps its bounds when `lo > hi`, so the high bound is set first;
/// setting the low bound on a dummy span would otherwise collapse back to zero.
fn span_at(lo: u32) -> Span { DUMMY_SP.with_hi(BytePos(lo + 1)).with_lo(BytePos(lo)) }

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> ModuleWorld { ModuleWorld::default() }

#[given("the module begins with an inner doc comment")]
fn given_inner_doc(world: &ModuleWorld) { world.push_inner_doc(10); }

#[given("the module body starts with code only")]
fn given_no_attributes(world: &ModuleWorld) {
    // No attributes are written in the body at all.
    let _ = world;
}

#[given("the module contains an inner configuration attribute")]
fn given_inner_allow(world: &ModuleWorld) { world.push_inner_other(10); }

#[given("documentation follows that attribute")]
fn given_doc_after(world: &ModuleWorld) { world.push_inner_doc(20); }

#[given("the module contains an inner configuration attribute and inline documentation")]
fn given_inline_attr_doc(world: &ModuleWorld) {
    // The doc record is pushed first to prove that HIR list order does not
    // decide the outcome; the source positions still put the lint attribute
    // first on the shared line.
    world.push_inner_doc(30);
    world.push_inner_other(10);
}

#[given("the module declares only outer documentation")]
fn given_outer_doc(world: &ModuleWorld) { world.push_outer_doc(5); }

#[when("I validate the module documentation requirements")]
fn when_detect(world: &ModuleWorld) { world.evaluate(); }

#[then("the module is accepted")]
fn then_accept(world: &ModuleWorld) {
    assert_eq!(world.outcome(), ModuleDocDisposition::HasLeadingDoc);
}

#[then("documentation is reported missing")]
fn then_missing(world: &ModuleWorld) {
    assert_eq!(world.outcome(), ModuleDocDisposition::MissingDocs);
}

#[then("documentation is reported after other attributes")]
fn then_misordered(world: &ModuleWorld) {
    assert!(matches!(
        world.outcome(),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
}

#[scenario(path = "tests/features/module_docs.feature", index = 0)]
fn scenario_docs_first(world: ModuleWorld) { let _ = world; }

#[scenario(path = "tests/features/module_docs.feature", index = 1)]
fn scenario_missing_docs(world: ModuleWorld) { let _ = world; }

#[scenario(path = "tests/features/module_docs.feature", index = 2)]
fn scenario_misordered_docs(world: ModuleWorld) { let _ = world; }

#[scenario(path = "tests/features/module_docs.feature", index = 3)]
fn scenario_inline_attr_doc(world: ModuleWorld) { let _ = world; }

#[scenario(path = "tests/features/module_docs.feature", index = 4)]
fn scenario_inner_attribute_only(world: ModuleWorld) { let _ = world; }

#[scenario(path = "tests/features/module_docs.feature", index = 5)]
fn scenario_outer_docs(world: ModuleWorld) { let _ = world; }
