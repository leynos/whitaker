//! Unbounded proof that Whitaker CLI token classification is total and disjoint.
//!
//! Run with `scripts/run-verus.sh cli`; the default `make verus` group omits
//! this proof until it verifies within tolerance. This model mirrors the pure
//! classifier in `src/cli/dispatch.rs`; the bounded Kani harness checks the
//! production implementation over token sequences up to length six.

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

/// Reports whether classification selects a Whitaker subcommand.
pub open spec fn is_subcommand(tokens: Seq<TokenTag>) -> bool {
    classify(tokens) == RoutingClass::Subcommand
}

/// Reports whether the first non-global token requests root help or version.
pub open spec fn is_display(tokens: Seq<TokenTag>) -> bool {
    classify(tokens) == RoutingClass::Display
}

/// Reports whether routing reaches the legacy `cargo dylint` path.
pub open spec fn is_forward(tokens: Seq<TokenTag>) -> bool {
    classify(tokens) == RoutingClass::Forward
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

/// Exactly one routing class applies to every finite token sequence.
proof fn lemma_classification_is_total_and_disjoint(tokens: Seq<TokenTag>)
    ensures
        is_subcommand(tokens) || is_display(tokens) || is_forward(tokens),
        !(is_subcommand(tokens) && is_forward(tokens)),
        !(is_display(tokens) && is_forward(tokens)),
        !(is_subcommand(tokens) && is_display(tokens)),
    decreases tokens.len()
{
    if tokens.len() == 0 {
        assert(is_forward(tokens));
    } else if tokens[0] == TokenTag::GlobalFlag {
        let rest = tokens.drop_first();
        lemma_classification_is_total_and_disjoint(rest);
        assert(classify(tokens) == classify(rest));
    } else if tokens[0] == TokenTag::Install || tokens[0] == TokenTag::Ls {
        assert(classify(tokens) == RoutingClass::Subcommand);
    } else if tokens[0] == TokenTag::Help || tokens[0] == TokenTag::Version {
        assert(classify(tokens) == RoutingClass::Display);
    } else {
        assert(classify(tokens) == RoutingClass::Forward);
    }
}

fn main() {
}

} // verus!
