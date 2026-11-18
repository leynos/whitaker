//! Unbounded proof that `whitaker check` keeps its own `--` first.
//!
//! Run with `scripts/run-verus.sh cli`; default `make verus` includes it.
//! This model mirrors `src/cli/check/argv.rs`: Whitaker's own options, then
//! its separator, then the user's arguments byte for byte. What the model
//! establishes is the property the forwarding relies on — no Whitaker-owned
//! argument is `--`, so the first separator in the assembled vector is
//! always Whitaker's, whatever the user forwarded, including another `--`.

use vstd::prelude::*;

verus! {

/// The options `whitaker check` owns, in the order it passes them.
pub enum DylintOption {
    /// Load exactly the Whitaker suite library.
    Lib,
    /// Ignore libraries discovered in the workspace metadata.
    NoMetadata,
}

/// A `cargo dylint` argument, as the model distinguishes them.
pub enum Argument {
    /// One of the options Whitaker owns.
    Owned(DylintOption),
    /// The end-of-options marker.
    Separator,
    /// An argument the user forwarded, which may itself be a separator.
    User,
}

/// Reports whether an argument is the end-of-options marker.
pub open spec fn is_separator(argument: Argument) -> bool {
    match argument {
        Argument::Separator => true,
        _ => false,
    }
}

/// The arguments Whitaker contributes before the user's own.
///
/// Every one is [`Argument::Owned`] up to the separator, which is the whole
/// reason the assembled vector's first separator can only be Whitaker's.
pub open spec fn whitaker_prefix() -> Seq<Argument> {
    seq![
        Argument::Owned(DylintOption::Lib),
        Argument::Owned(DylintOption::NoMetadata),
        Argument::Separator,
    ]
}

/// How many arguments Whitaker contributes, including its separator.
pub open spec fn whitaker_prefix_len() -> int {
    3
}

/// Assemble the arguments `cargo dylint` receives: `W ++ ["--"] ++ U`.
pub open spec fn assemble(user: Seq<Argument>) -> Seq<Argument> {
    whitaker_prefix() + user
}

/// The prefix Whitaker contributes is fixed, whatever the user forwarded.
proof fn lemma_assembly_starts_with_whitakers_arguments(user: Seq<Argument>)
    ensures
        assemble(user).len() == whitaker_prefix_len() + user.len(),
        assemble(user)[0] == Argument::Owned(DylintOption::Lib),
        assemble(user)[1] == Argument::Owned(DylintOption::NoMetadata),
        assemble(user)[2] == Argument::Separator,
{
    let assembled = assemble(user);
    reveal(assemble);
    reveal(whitaker_prefix);
    assert(assembled[0] == Argument::Owned(DylintOption::Lib));
    assert(assembled[1] == Argument::Owned(DylintOption::NoMetadata));
    assert(assembled[2] == Argument::Separator);
}

/// Only Whitaker's own options precede the separator, and none of them is one.
proof fn lemma_nothing_before_the_separator_is_a_separator(user: Seq<Argument>)
    ensures
        !is_separator(assemble(user)[0]),
        !is_separator(assemble(user)[1]),
{
    lemma_assembly_starts_with_whitakers_arguments(user);
    assert(!is_separator(Argument::Owned(DylintOption::Lib)));
    assert(!is_separator(Argument::Owned(DylintOption::NoMetadata)));
}

/// The user's arguments appear after the separator, unchanged and in order.
proof fn lemma_assembly_preserves_the_user_suffix(user: Seq<Argument>)
    ensures
        assemble(user).subrange(whitaker_prefix_len(), assemble(user).len() as int) == user,
{
    reveal(assemble);
    reveal(whitaker_prefix);
}

/// A user `--` survives assembly, at index three, and is not mistaken for
/// Whitaker's own separator at index two.
proof fn lemma_a_user_separator_is_forwarded()
    ensures
        assemble(seq![Argument::Separator])[whitaker_prefix_len()] == Argument::Separator,
        assemble(seq![Argument::Separator])[2] == Argument::Separator,
{
    lemma_assembly_starts_with_whitakers_arguments(seq![Argument::Separator]);
    reveal(assemble);
    assert(assemble(seq![Argument::Separator])[whitaker_prefix_len()] == Argument::Separator);
}

/// Provide the entry point Verus expects when it compiles this proof crate.
///
/// The proofs are checked at compile time, so there is nothing to run.
fn main() {}

} // verus!
