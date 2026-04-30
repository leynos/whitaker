//! Behaviour-driven tests for shared `rstest` fingerprint models.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use whitaker_common::rstest::{
    ArgAtom, ArgFingerprint, CalleeShape, ExprShape, LocalSlot, ParagraphFingerprint,
    ParagraphNormalizer, StmtShape,
};

#[derive(Default)]
struct FingerprintWorld {
    first_args: RefCell<Option<ArgFingerprint>>,
    second_args: RefCell<Option<ArgFingerprint>>,
    first_paragraph: RefCell<Option<ParagraphFingerprint>>,
    second_paragraph: RefCell<Option<ParagraphFingerprint>>,
    args_match: RefCell<Option<bool>>,
    paragraphs_match: RefCell<Option<bool>>,
}

impl FingerprintWorld {
    fn set_first_args(&self, fingerprint: ArgFingerprint) {
        self.first_args.replace(Some(fingerprint));
    }

    fn set_second_args(&self, fingerprint: ArgFingerprint) {
        self.second_args.replace(Some(fingerprint));
    }

    fn set_first_paragraph(&self, fingerprint: ParagraphFingerprint) {
        self.first_paragraph.replace(Some(fingerprint));
    }

    fn set_second_paragraph(&self, fingerprint: ParagraphFingerprint) {
        self.second_paragraph.replace(Some(fingerprint));
    }

    fn compare_args(&self) {
        self.args_match.replace(Some(
            *self.first_args.borrow() == *self.second_args.borrow(),
        ));
    }

    fn compare_paragraphs(&self) {
        self.paragraphs_match.replace(Some(
            *self.first_paragraph.borrow() == *self.second_paragraph.borrow(),
        ));
    }
}

#[fixture]
fn world() -> FingerprintWorld {
    FingerprintWorld::default()
}

#[given("helper-call arguments for fixture db and literal 42")]
fn given_helper_args(world: &FingerprintWorld) {
    world.set_first_args(helper_args());
}

#[given("matching helper-call arguments for fixture db and literal 42")]
fn given_matching_helper_args(world: &FingerprintWorld) {
    world.set_second_args(helper_args());
}

#[given("helper-call arguments containing an unsupported argument")]
fn given_unsupported_args(world: &FingerprintWorld) {
    world.set_first_args(ArgFingerprint::new([
        ArgAtom::fixture_local("db"),
        ArgAtom::unsupported(),
    ]));
}

#[given("a setup paragraph using locals {first} and {second}")]
fn given_setup_paragraph(world: &FingerprintWorld, first: String, second: String) {
    world.set_first_paragraph(setup_paragraph(&first, &second, 1));
}

#[given("a matching setup paragraph using locals {first} and {second}")]
fn given_matching_setup_paragraph(world: &FingerprintWorld, first: String, second: String) {
    world.set_second_paragraph(setup_paragraph(&first, &second, 1));
}

#[given("a setup paragraph with a one-argument constructor")]
fn given_one_arg_paragraph(world: &FingerprintWorld) {
    world.set_first_paragraph(setup_paragraph("user", "cache", 1));
}

#[given("a matching setup paragraph with a two-argument constructor")]
fn given_two_arg_paragraph(world: &FingerprintWorld) {
    world.set_second_paragraph(setup_paragraph("account", "store", 2));
}

#[when("I compare the argument fingerprints")]
fn when_compare_args(world: &FingerprintWorld) {
    world.compare_args();
}

#[when("I compare the paragraph fingerprints")]
fn when_compare_paragraphs(world: &FingerprintWorld) {
    world.compare_paragraphs();
}

#[when("I inspect the argument fingerprint")]
fn when_inspect_args(world: &FingerprintWorld) {
    let _ = world;
}

#[when("I inspect the paragraph fingerprint")]
fn when_inspect_paragraph(world: &FingerprintWorld) {
    let _ = world;
}

#[then("the argument fingerprints match")]
fn then_args_match(world: &FingerprintWorld) {
    assert_eq!(*world.args_match.borrow(), Some(true));
}

#[then("the paragraph fingerprints match")]
fn then_paragraphs_match(world: &FingerprintWorld) {
    assert_eq!(*world.paragraphs_match.borrow(), Some(true));
}

#[then("the unsupported argument is still present")]
fn then_unsupported_argument_is_present(world: &FingerprintWorld) {
    let fingerprint = world.first_args.borrow();
    let atoms = fingerprint
        .as_ref()
        .map(ArgFingerprint::atoms)
        .unwrap_or_default();

    assert!(atoms.contains(&ArgAtom::unsupported()));
}

#[then("the paragraph fingerprints differ")]
fn then_paragraphs_differ(world: &FingerprintWorld) {
    assert_eq!(*world.paragraphs_match.borrow(), Some(false));
}

#[then("zeta has slot 0 and alpha has slot 1")]
fn then_slots_follow_first_appearance(world: &FingerprintWorld) {
    let fingerprint = world.first_paragraph.borrow();
    let shapes = fingerprint
        .as_ref()
        .map(ParagraphFingerprint::shapes)
        .unwrap_or_default();

    assert_eq!(
        shapes,
        &[
            StmtShape::let_binding(ExprShape::call(CalleeShape::def_path("crate::build"), 1)),
            StmtShape::mutable_call(Some(LocalSlot::new(0)), CalleeShape::unknown()),
            StmtShape::mutable_call(Some(LocalSlot::new(1)), CalleeShape::unknown()),
        ]
    );
}

fn helper_args() -> ArgFingerprint {
    ArgFingerprint::new([ArgAtom::fixture_local("db"), ArgAtom::const_lit("42")])
}

fn setup_paragraph(first: &str, second: &str, constructor_argc: usize) -> ParagraphFingerprint {
    let mut normalizer = ParagraphNormalizer::new();
    ParagraphFingerprint::new([
        StmtShape::let_binding(ExprShape::call(
            CalleeShape::def_path("crate::build"),
            constructor_argc,
        )),
        StmtShape::mutable_call(Some(normalizer.local_slot(first)), CalleeShape::unknown()),
        StmtShape::mutable_call(Some(normalizer.local_slot(second)), CalleeShape::unknown()),
    ])
}

#[scenario(path = "tests/features/rstest_fingerprint.feature", index = 0)]
fn scenario_equivalent_arguments_match(world: FingerprintWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/rstest_fingerprint.feature", index = 1)]
fn scenario_renamed_paragraphs_match(world: FingerprintWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/rstest_fingerprint.feature", index = 2)]
fn scenario_unsupported_arguments_remain_explicit(world: FingerprintWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/rstest_fingerprint.feature", index = 3)]
fn scenario_structural_paragraphs_diverge(world: FingerprintWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/rstest_fingerprint.feature", index = 4)]
fn scenario_first_appearance_order_controls_slots(world: FingerprintWorld) {
    let _ = world;
}
