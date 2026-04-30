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
