//! Unit tests for shared `rstest` fingerprint data models.

use crate::rstest::{
    ArgAtom, ArgFingerprint, CalleeShape, ExprShape, LocalSlot, ParagraphFingerprint,
    ParagraphNormalizer, StmtShape,
};
use rstest::rstest;

#[rstest]
fn argument_fingerprints_compare_identical_atom_sequences() {
    let first = ArgFingerprint::new([
        ArgAtom::fixture_local("db"),
        ArgAtom::const_lit("42"),
        ArgAtom::const_path("crate::defaults::TIMEOUT"),
    ]);
    let second = ArgFingerprint::new([
        ArgAtom::fixture_local("db"),
        ArgAtom::const_lit("42"),
        ArgAtom::const_path("crate::defaults::TIMEOUT"),
    ]);

    assert_eq!(first, second);
}

#[rstest]
fn argument_fingerprints_preserve_positional_differences() {
    let fixture_then_literal =
        ArgFingerprint::new([ArgAtom::fixture_local("db"), ArgAtom::const_lit("42")]);
    let literal_then_fixture =
        ArgFingerprint::new([ArgAtom::const_lit("42"), ArgAtom::fixture_local("db")]);

    assert_ne!(fixture_then_literal, literal_then_fixture);
}

#[rstest]
fn unsupported_argument_atoms_remain_present() {
    let fingerprint = ArgFingerprint::new([
        ArgAtom::fixture_local("db"),
        ArgAtom::unsupported(),
        ArgAtom::const_lit("42"),
    ]);

    assert_eq!(
        fingerprint.atoms(),
        &[
            ArgAtom::fixture_local("db"),
            ArgAtom::unsupported(),
            ArgAtom::const_lit("42"),
        ]
    );
}

#[rstest]
fn paragraph_fingerprints_normalize_renamed_locals_by_first_appearance() {
    let first = paragraph_for_renamed_locals("user", "cache");
    let second = paragraph_for_renamed_locals("account", "store");

    assert_eq!(first, second);
}

#[rstest]
fn paragraph_fingerprints_diverge_for_structural_differences() {
    let two_argument_call = ParagraphFingerprint::new([StmtShape::let_binding(ExprShape::call(
        CalleeShape::def_path("crate::make_user"),
        2,
    ))]);
    let one_argument_call = ParagraphFingerprint::new([StmtShape::let_binding(ExprShape::call(
        CalleeShape::def_path("crate::make_user"),
        1,
    ))]);

    assert_ne!(two_argument_call, one_argument_call);
}

#[rstest]
fn paragraph_normalization_is_deterministic_across_runs() {
    let first = paragraph_for_renamed_locals("zeta", "alpha");
    let second = paragraph_for_renamed_locals("zeta", "alpha");

    assert_eq!(first, second);
    assert_eq!(
        first.shapes(),
        &[
            StmtShape::let_binding(ExprShape::call(CalleeShape::def_path("crate::load"), 0)),
            StmtShape::mutable_call(
                Some(LocalSlot::new(0)),
                CalleeShape::def_path("crate::prepare"),
            ),
            StmtShape::mutable_call(
                Some(LocalSlot::new(1)),
                CalleeShape::def_path("crate::prepare"),
            ),
        ]
    );
}

#[rstest]
fn empty_argument_fingerprint_equals_another_empty() {
    let first = ArgFingerprint::new([]);
    let second = ArgFingerprint::new([]);
    assert_eq!(first, second);
    assert!(first.atoms().is_empty());
}

#[rstest]
fn empty_paragraph_fingerprint_equals_another_empty() {
    let first = ParagraphFingerprint::new([]);
    let second = ParagraphFingerprint::new([]);
    assert_eq!(first, second);
    assert!(first.shapes().is_empty());
}

#[rstest]
fn arg_atom_constructors_accept_empty_string() {
    let fl = ArgAtom::fixture_local("");
    let cl = ArgAtom::const_lit("");
    let cp = ArgAtom::const_path("");
    assert_eq!(fl, ArgAtom::fixture_local(""));
    assert_eq!(cl, ArgAtom::const_lit(""));
    assert_eq!(cp, ArgAtom::const_path(""));
}

#[rstest]
fn arg_atom_constructors_accept_long_string() {
    let long: String = "x".repeat(4096);
    let atom = ArgAtom::fixture_local(&long);
    assert_eq!(atom, ArgAtom::fixture_local(long));
}

#[rstest]
fn local_slot_new_roundtrips_index() {
    assert_eq!(LocalSlot::new(0).index(), 0);
    assert_eq!(LocalSlot::new(u32::MAX).index(), u32::MAX);
}

#[rstest]
fn paragraph_normalizer_returns_same_slot_for_repeated_name() {
    let mut norm = ParagraphNormalizer::new();
    let first = norm.local_slot("foo");
    let second = norm.local_slot("foo");
    assert_eq!(first, second);
}

#[rstest]
fn paragraph_normalizer_assigns_slots_in_first_appearance_order() {
    let mut norm = ParagraphNormalizer::new();
    let zeta = norm.local_slot("zeta");
    let alpha = norm.local_slot("alpha");
    assert_eq!(zeta.index(), 0);
    assert_eq!(alpha.index(), 1);
}

fn paragraph_for_renamed_locals(first_name: &str, second_name: &str) -> ParagraphFingerprint {
    let mut normalizer = ParagraphNormalizer::new();
    ParagraphFingerprint::new([
        StmtShape::let_binding(ExprShape::call(CalleeShape::def_path("crate::load"), 0)),
        StmtShape::mutable_call(
            Some(normalizer.local_slot(first_name)),
            CalleeShape::def_path("crate::prepare"),
        ),
        StmtShape::mutable_call(
            Some(normalizer.local_slot(second_name)),
            CalleeShape::def_path("crate::prepare"),
        ),
    ])
}
