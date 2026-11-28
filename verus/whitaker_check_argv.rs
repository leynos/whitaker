//! Unbounded proof that `whitaker check` keeps its own `--` first.
//!
//! Run with `scripts/run-verus.sh cli`; default `make verus` includes it.
//! This model mirrors `src/cli/check/argv.rs`: Whitaker's own options, then
//! its separator, then the user's arguments byte for byte.
//!
//! The proof is stated over any sequence `W` of Whitaker-owned arguments that
//! contains no separator, and any user sequence `U` — not only over the two
//! options `check` passes today. That is the plan's lemma: the first
//! separator of `assemble(W, U)` is at index `|W|`, and everything after it
//! is `U`, so a `W` that grew an option could not shift the boundary. The
//! concrete two-option vector is then a corollary, because it is one such `W`.
//!
//! The model preserves the count and position of abstract tokens; every
//! forwarded token is the same payload-free `Argument::User`, so it cannot
//! distinguish two of them. It does not model how the Rust code renders an
//! option into bytes or copies a user token; those properties are covered by
//! the unit and property tests.

use vstd::prelude::*;

verus! {

/// The options `whitaker check` owns, in the order it passes them.
pub enum DylintOption {
    /// Load exactly the Whitaker suite library.
    Lib,
    /// Ignore libraries discovered in the workspace metadata.
    NoMetadata,
    /// Read the workspace from the manifest the caller named.
    ///
    /// The option is emitted only when `--manifest-path` was given, and it is
    /// Dylint's own option rather than a Cargo argument: Dylint reads its
    /// target directory from the manifest before it forwards anything after
    /// the separator.
    ManifestPath,
}

/// A `cargo dylint` argument, as the model distinguishes them.
pub enum Argument {
    /// One of the options Whitaker owns.
    Owned(DylintOption),
    /// The end-of-options marker.
    Separator,
    /// An opaque token the user forwarded that is not a separator.
    ///
    /// A user's own `--` is modelled as [`Argument::Separator`], because the
    /// property under proof distinguishes separators from other tokens. Only
    /// the position and identity of each token are modelled, not its bytes.
    User,
}

/// Test whether an argument is the end-of-options marker.
pub open spec fn is_separator(argument: Argument) -> bool {
    match argument {
        Argument::Separator => true,
        _ => false,
    }
}

/// The options `whitaker check` always passes before its separator.
pub open spec fn whitaker_options() -> Seq<Argument> {
    seq![
        Argument::Owned(DylintOption::Lib),
        Argument::Owned(DylintOption::NoMetadata),
    ]
}

/// The options `whitaker check` passes when the caller names a manifest.
pub open spec fn whitaker_options_with_manifest() -> Seq<Argument> {
    whitaker_options() + seq![Argument::Owned(DylintOption::ManifestPath)]
}

/// Assemble the arguments `cargo dylint` receives: `W ++ ["--"] ++ U`.
pub open spec fn assemble(options: Seq<Argument>, user: Seq<Argument>) -> Seq<Argument> {
    options + seq![Argument::Separator] + user
}

/// The least index holding a separator, or the length when there is none.
pub open spec fn first_separator(arguments: Seq<Argument>) -> int
    decreases arguments.len(),
{
    if arguments.len() == 0 {
        0
    } else if is_separator(arguments[0]) {
        0
    } else {
        1 + first_separator(arguments.subrange(1, arguments.len() as int))
    }
}

/// The arguments after the first separator, as `cargo dylint` receives them.
pub open spec fn cargo_args(arguments: Seq<Argument>) -> Seq<Argument>
    decreases arguments.len(),
{
    if arguments.len() == 0 {
        Seq::empty()
    } else if is_separator(arguments[0]) {
        arguments.subrange(1, arguments.len() as int)
    } else {
        cargo_args(arguments.subrange(1, arguments.len() as int))
    }
}

/// Report whether every argument in `arguments` is one Whitaker owns.
///
/// This is the model's statement of "`W` without `--`": an owned argument is
/// never the end-of-options marker.
pub open spec fn has_no_separator(arguments: Seq<Argument>) -> bool {
    forall|index: int| 0 <= index < arguments.len() ==> !is_separator(#[trigger] arguments[index])
}

/// Prove that Whitaker's separator sits at `|W|`, whatever the user sent.
///
/// `U` is unconstrained, so it may itself contain a separator: only the first
/// one is the boundary, and the first one is Whitaker's.
pub proof fn lemma_assembly_starts_with_whitakers_arguments(
    options: Seq<Argument>,
    user: Seq<Argument>,
)
    requires has_no_separator(options),
    ensures first_separator(assemble(options, user)) == options.len(),
    decreases options.len(),
{
    let assembled = assemble(options, user);
    if options.len() == 0 {
        assert(assembled =~= seq![Argument::Separator] + user);
        assert(is_separator(assembled[0]));
    } else {
        assert(assembled[0] == options[0]);
        assert(!is_separator(assembled[0]));
        lemma_assembly_starts_with_whitakers_arguments(
            options.subrange(1, options.len() as int),
            user,
        );
        assert(assembled.subrange(1, assembled.len() as int)
            =~= assemble(options.subrange(1, options.len() as int), user));
    }
}

/// Prove that `cargo dylint` receives exactly the user's arguments.
///
/// The statement is over the same arbitrary `W` and `U`, so it holds for any
/// growth of Whitaker's option set rather than for today's two options.
pub proof fn lemma_assembly_preserves_the_user_suffix(
    options: Seq<Argument>,
    user: Seq<Argument>,
)
    requires has_no_separator(options),
    ensures cargo_args(assemble(options, user)) == user,
    decreases options.len(),
{
    let assembled = assemble(options, user);
    if options.len() == 0 {
        assert(assembled =~= seq![Argument::Separator] + user);
        assert(is_separator(assembled[0]));
        assert(assembled.subrange(1, assembled.len() as int) =~= user);
    } else {
        assert(assembled[0] == options[0]);
        assert(!is_separator(assembled[0]));
        assert(assembled.subrange(1, assembled.len() as int)
            =~= assemble(options.subrange(1, options.len() as int), user));
        lemma_assembly_preserves_the_user_suffix(
            options.subrange(1, options.len() as int),
            user,
        );
    }
}

/// Prove that no Whitaker option is itself the separator.
///
/// This is the precondition the two lemmas above rely on, stated for the
/// concrete option set `check` passes.
pub proof fn lemma_nothing_before_the_separator_is_a_separator()
    ensures
        has_no_separator(whitaker_options()),
        has_no_separator(whitaker_options_with_manifest()),
        !is_separator(Argument::Owned(DylintOption::Lib)),
        !is_separator(Argument::Owned(DylintOption::NoMetadata)),
        !is_separator(Argument::Owned(DylintOption::ManifestPath)),
{
    assert(!is_separator(Argument::Owned(DylintOption::Lib)));
    assert(!is_separator(Argument::Owned(DylintOption::NoMetadata)));
    assert(!is_separator(Argument::Owned(DylintOption::ManifestPath)));
}

/// Prove that a user `--` survives after Whitaker's own, and is not confused
/// with it.
///
/// The boundary stays at `|W|` even when `U` is itself a separator: Whitaker's
/// marker is still the first one, and the user's is still forwarded.
pub proof fn lemma_a_user_separator_is_forwarded(user: Seq<Argument>)
    requires has_no_separator(whitaker_options()),
    ensures
        first_separator(assemble(whitaker_options(), user)) == whitaker_options().len(),
        cargo_args(assemble(whitaker_options(), user)) == user,
        first_separator(assemble(whitaker_options(), seq![Argument::Separator])) == 2,
        is_separator(assemble(whitaker_options(), seq![Argument::Separator])[3]),
{
    lemma_nothing_before_the_separator_is_a_separator();
    lemma_assembly_starts_with_whitakers_arguments(whitaker_options(), user);
    lemma_assembly_preserves_the_user_suffix(whitaker_options(), user);
    // The two lemmas above hold for every user sequence, so the concrete
    // separator-only sequence is a second instance rather than a new proof.
    lemma_assembly_starts_with_whitakers_arguments(
        whitaker_options(),
        seq![Argument::Separator],
    );
    let with_user_separator = assemble(whitaker_options(), seq![Argument::Separator]);
    assert(whitaker_options().len() == 2);
    assert(with_user_separator.len() == 4);
    assert(with_user_separator[3] == Argument::Separator);
}

/// Prove that naming a manifest keeps Whitaker's boundary and the user suffix.
///
/// `--manifest-path=<path>` is one Whitaker-owned argument and is never the
/// separator, so the general lemmas cover it. This instance pins the concrete
/// three-option prefix `check` emits when the caller names a manifest.
pub proof fn lemma_a_manifest_option_keeps_the_boundary(user: Seq<Argument>)
    ensures
        has_no_separator(whitaker_options_with_manifest()),
        first_separator(assemble(whitaker_options_with_manifest(), user))
            == whitaker_options_with_manifest().len(),
        cargo_args(assemble(whitaker_options_with_manifest(), user)) == user,
{
    lemma_nothing_before_the_separator_is_a_separator();
    lemma_assembly_starts_with_whitakers_arguments(whitaker_options_with_manifest(), user);
    lemma_assembly_preserves_the_user_suffix(whitaker_options_with_manifest(), user);
    assert(whitaker_options_with_manifest().len() == 3);
}

/// Provide the entry point Verus expects when it compiles this proof crate.
///
/// The proofs are checked at compile time, so there is nothing to run.
fn main() {}

} // verus!