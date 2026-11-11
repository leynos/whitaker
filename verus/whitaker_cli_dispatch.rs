//! Unbounded proof that Whitaker CLI token classification is total and disjoint.
//!
//! Run with `scripts/run-verus.sh cli`; default `make verus` includes it.
//! This model mirrors `src/cli/dispatch.rs`; bounded Kani checks the production
//! implementation over token sequences up to length six.

use vstd::prelude::*;

verus! {

/// Token categories inspected by the Whitaker CLI router.
pub enum TokenTag {
    /// Root `install` command.
    Install,
    /// Root `ls` command.
    Ls,
    /// Root help option.
    Help,
    /// Root version option.
    Version,
    /// End-of-options marker.
    DoubleDash,
    /// Shared quiet or verbose option.
    GlobalFlag,
    /// Option owned by `cargo dylint`.
    DylintFlag,
    /// Any other token.
    Other,
}

/// The single route selected by the root CLI classifier.
pub enum RoutingClass {
    /// Dispatch to an installer or inventory subcommand.
    Subcommand,
    /// Print root-level help or version output.
    Display,
    /// Forward the arguments to `cargo dylint`.
    Forward,
}

/// Classifies tokens using the same first-significant-token policy as Rust.
pub open spec fn classify(tokens: Seq<TokenTag>) -> RoutingClass
    decreases tokens.len()
{
    if tokens.len() == 0 {
        RoutingClass::Forward
    } else if tokens[0] == TokenTag::GlobalFlag {
        classify(tokens.drop_first())
    } else if tokens[0] == TokenTag::Install || tokens[0] == TokenTag::Ls {
        RoutingClass::Subcommand
    } else if tokens[0] == TokenTag::Help || tokens[0] == TokenTag::Version {
        RoutingClass::Display
    } else {
        RoutingClass::Forward
    }
}

/// Reports whether the first token, skipping global flags, is a subcommand.
///
/// Defined from the input alone, not from [`classify`]: if these predicates
/// merely restated the classifier's result, totality and disjointness would be
/// true by construction and would prove nothing about routing.
pub open spec fn is_subcommand(tokens: Seq<TokenTag>) -> bool
    decreases tokens.len()
{
    if tokens.len() == 0 {
        false
    } else if tokens[0] == TokenTag::GlobalFlag {
        is_subcommand(tokens.drop_first())
    } else {
        tokens[0] == TokenTag::Install || tokens[0] == TokenTag::Ls
    }
}

/// Reports whether the first non-global token requests root help or version.
pub open spec fn is_display(tokens: Seq<TokenTag>) -> bool
    decreases tokens.len()
{
    if tokens.len() == 0 {
        false
    } else if tokens[0] == TokenTag::GlobalFlag {
        is_display(tokens.drop_first())
    } else {
        tokens[0] == TokenTag::Help || tokens[0] == TokenTag::Version
    }
}

/// Reports whether the first non-global token reaches the legacy path.
///
/// A sequence made only of global flags has no first significant token, and
/// routing forwards it, so an exhausted sequence is a forwarding input.
pub open spec fn is_forward(tokens: Seq<TokenTag>) -> bool
    decreases tokens.len()
{
    if tokens.len() == 0 {
        true
    } else if tokens[0] == TokenTag::GlobalFlag {
        is_forward(tokens.drop_first())
    } else {
        tokens[0] != TokenTag::Install
            && tokens[0] != TokenTag::Ls
            && tokens[0] != TokenTag::Help
            && tokens[0] != TokenTag::Version
    }
}

/// Each routing class has an explicit witness, preventing vacuous disjointness.
proof fn lemma_classes_are_inhabited()
    ensures
        exists|tokens: Seq<TokenTag>| is_subcommand(tokens),
        exists|tokens: Seq<TokenTag>| is_display(tokens),
        exists|tokens: Seq<TokenTag>| is_forward(tokens),
{
    assert(is_subcommand(seq![TokenTag::Install])) by (compute);
    assert(is_display(seq![TokenTag::Help])) by (compute);
    assert(is_forward(seq![TokenTag::DylintFlag])) by (compute);
}

/// Exactly one input predicate holds for every finite token sequence.
///
/// This is a property of the predicates themselves: whichever token ends the
/// run of global flags decides the class, and the four deciding tokens are
/// distinct. Proving it here means the disjointness below cannot be true
/// merely because the predicates were written in terms of one classifier.
proof fn lemma_predicates_partition_the_inputs(tokens: Seq<TokenTag>)
    ensures
        is_subcommand(tokens) || is_display(tokens) || is_forward(tokens),
        !(is_subcommand(tokens) && is_forward(tokens)),
        !(is_display(tokens) && is_forward(tokens)),
        !(is_subcommand(tokens) && is_display(tokens)),
    decreases tokens.len()
{
    if tokens.len() == 0 {
        assert(is_forward(tokens));
        assert(!is_subcommand(tokens));
        assert(!is_display(tokens));
    } else if tokens[0] == TokenTag::GlobalFlag {
        let rest = tokens.drop_first();
        lemma_predicates_partition_the_inputs(rest);
        assert(is_subcommand(tokens) == is_subcommand(rest));
        assert(is_display(tokens) == is_display(rest));
        assert(is_forward(tokens) == is_forward(rest));
    } else if tokens[0] == TokenTag::Install || tokens[0] == TokenTag::Ls {
        assert(is_subcommand(tokens));
        assert(!is_display(tokens));
        assert(!is_forward(tokens));
    } else if tokens[0] == TokenTag::Help || tokens[0] == TokenTag::Version {
        assert(is_display(tokens));
        assert(!is_subcommand(tokens));
        assert(!is_forward(tokens));
    } else {
        assert(is_forward(tokens));
        assert(!is_subcommand(tokens));
        assert(!is_display(tokens));
    }
}

/// The production classifier agrees with the input predicates everywhere.
///
/// This is the bridge between [`classify`], which models `src/cli/dispatch.rs`,
/// and the predicates above. Together with
/// [`lemma_predicates_partition_the_inputs`] it establishes that the modelled
/// router assigns exactly one class to every finite token sequence.
proof fn lemma_classification_agrees_with_predicates(tokens: Seq<TokenTag>)
    ensures
        is_subcommand(tokens) == (classify(tokens) == RoutingClass::Subcommand),
        is_display(tokens) == (classify(tokens) == RoutingClass::Display),
        is_forward(tokens) == (classify(tokens) == RoutingClass::Forward),
    decreases tokens.len()
{
    if tokens.len() == 0 {
        assert(classify(tokens) == RoutingClass::Forward);
        assert(is_forward(tokens));
        assert(!is_subcommand(tokens));
        assert(!is_display(tokens));
    } else if tokens[0] == TokenTag::GlobalFlag {
        let rest = tokens.drop_first();
        lemma_classification_agrees_with_predicates(rest);
        assert(classify(tokens) == classify(rest));
        assert(is_subcommand(tokens) == is_subcommand(rest));
        assert(is_display(tokens) == is_display(rest));
        assert(is_forward(tokens) == is_forward(rest));
    } else if tokens[0] == TokenTag::Install || tokens[0] == TokenTag::Ls {
        assert(classify(tokens) == RoutingClass::Subcommand);
        assert(is_subcommand(tokens));
        assert(!is_display(tokens));
        assert(!is_forward(tokens));
    } else if tokens[0] == TokenTag::Help || tokens[0] == TokenTag::Version {
        assert(classify(tokens) == RoutingClass::Display);
        assert(!is_subcommand(tokens));
        assert(is_display(tokens));
        assert(!is_forward(tokens));
    } else {
        assert(classify(tokens) == RoutingClass::Forward);
        assert(!is_subcommand(tokens));
        assert(!is_display(tokens));
        assert(is_forward(tokens));
    }
}

/// Provide the entry point Verus expects when it compiles this proof crate.
///
/// The proofs are checked at compile time, so there is nothing to run.
fn main() {}

} // verus!
