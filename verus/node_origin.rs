//! Verus model for classifying HIR node provenance from expansion marks.
//!
//! This proof relates the executable early-exit scan used by
//! `whitaker_common::node_origin::classify` to an independent sequence
//! specification. It does not verify rustc hygiene, the adapter that builds
//! mark chains from `SyntaxContext`, or arbitrary Rust code; those boundaries
//! remain covered by the compiler-backed tests.

use vstd::prelude::*;

verus! {

broadcast use vstd::seq::group_seq_axioms;

/// One expansion mark on a syntax context.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// A compiler desugaring, which leaves the node authored.
    Desugaring,
    /// A macro expansion, which makes the node generated.
    Macro,
    /// A compiler AST pass, which makes the node generated.
    AstPass,
}

/// Where a HIR node came from.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Written by a person in crate source.
    Authored,
    /// Produced by a macro expansion or compiler AST pass.
    Generated,
}

/// Reports whether one mark alone proves generated provenance.
pub open spec fn is_generated_mark(mark: Mark) -> bool {
    match mark {
        Mark::Desugaring => false,
        Mark::Macro => true,
        Mark::AstPass => true,
    }
}

/// The independent specification: generated exactly when some mark is
/// generated.
pub open spec fn spec_origin(marks: Seq<Mark>) -> Origin {
    if exists|index: int|
        0 <= index < marks.len() && #[trigger] is_generated_mark(marks[index])
    {
        Origin::Generated
    } else {
        Origin::Authored
    }
}

/// Classifies marks by scanning from the outermost mark inwards.
///
/// This mirrors the production early exit: a desugaring is transparent, and
/// the first macro or AST-pass mark makes the node generated.
pub fn scan(marks: &[Mark]) -> (origin: Origin)
    ensures
        origin == spec_origin(marks@),
{
    let mut index: usize = 0;
    while index < marks.len()
        invariant
            index <= marks.len(),
            forall|earlier: int| #![trigger marks@[earlier]]
                0 <= earlier < index ==> marks@[earlier] == Mark::Desugaring,
        decreases marks.len() - index,
    {
        let mark = marks[index];
        match mark {
            Mark::Desugaring => {
                index = index + 1;
            }
            Mark::Macro => {
                assert(is_generated_mark(marks@[index as int]));
                return Origin::Generated;
            }
            Mark::AstPass => {
                assert(is_generated_mark(marks@[index as int]));
                return Origin::Generated;
            }
        }
    }
    assert(index == marks.len());
    assert(forall|earlier: int| #![trigger marks@[earlier]]
        0 <= earlier < marks@.len() ==> marks@[earlier] == Mark::Desugaring);
    Origin::Authored
}

/// Both classes have concrete, non-empty witnesses.
pub proof fn lemma_class_witnesses()
    ensures
        spec_origin(Seq::<Mark>::empty()) == Origin::Authored,
        spec_origin(seq![Mark::Desugaring]) == Origin::Authored,
        spec_origin(seq![Mark::Desugaring, Mark::Macro]) == Origin::Generated,
        spec_origin(seq![Mark::Desugaring, Mark::AstPass]) == Origin::Generated,
{
    let macro_witness = seq![Mark::Desugaring, Mark::Macro];
    let ast_pass_witness = seq![Mark::Desugaring, Mark::AstPass];
    assert(spec_origin(Seq::<Mark>::empty()) == Origin::Authored);
    assert(spec_origin(seq![Mark::Desugaring]) == Origin::Authored);
    assert(is_generated_mark(macro_witness[1]));
    assert(spec_origin(macro_witness) == Origin::Generated);
    assert(is_generated_mark(ast_pass_witness[1]));
    assert(spec_origin(ast_pass_witness) == Origin::Generated);
}

/// Concatenation is generated exactly when either component is generated.
pub proof fn lemma_concatenation(prefix: Seq<Mark>, suffix: Seq<Mark>)
    ensures
        spec_origin(prefix + suffix) == Origin::Generated
            <==> (spec_origin(prefix) == Origin::Generated
                || spec_origin(suffix) == Origin::Generated),
{
    if spec_origin(prefix + suffix) == Origin::Generated {
        let index = choose|index: int|
            0 <= index < (prefix + suffix).len()
                && #[trigger] is_generated_mark((prefix + suffix)[index]);
        if index < prefix.len() {
            assert(is_generated_mark(prefix[index]));
            assert(spec_origin(prefix) == Origin::Generated);
        } else {
            assert(is_generated_mark(suffix[index - prefix.len()]));
            assert(spec_origin(suffix) == Origin::Generated);
        }
    }
    if spec_origin(prefix) == Origin::Generated {
        let index = choose|index: int|
            0 <= index < prefix.len() && #[trigger] is_generated_mark(prefix[index]);
        assert(is_generated_mark((prefix + suffix)[index]));
        assert(spec_origin(prefix + suffix) == Origin::Generated);
    }
    if spec_origin(suffix) == Origin::Generated {
        let index = choose|index: int|
            0 <= index < suffix.len() && #[trigger] is_generated_mark(suffix[index]);
        assert(is_generated_mark((prefix + suffix)[prefix.len() + index]));
        assert(spec_origin(prefix + suffix) == Origin::Generated);
    }
}

/// Inserting a desugaring anywhere leaves the origin unchanged.
pub proof fn lemma_desugaring_is_transparent(prefix: Seq<Mark>, suffix: Seq<Mark>)
    ensures
        spec_origin(prefix + seq![Mark::Desugaring] + suffix)
            == spec_origin(prefix + suffix),
{
    let singleton = seq![Mark::Desugaring];
    lemma_concatenation(prefix, singleton);
    lemma_concatenation(prefix + singleton, suffix);
    lemma_concatenation(prefix, suffix);
    assert(spec_origin(singleton) == Origin::Authored);
}

fn main() {
}

} // verus!