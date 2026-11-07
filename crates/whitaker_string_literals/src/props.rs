//! Shared `proptest` strategies for the continuation and rewrite suites.
//!
//! Both suites need to generate the same shapes: a body built from source
//! fragments with a continuation between each pair. They differ only in the
//! fragment alphabet, because the rewrite suite decodes the generated value
//! and so needs escapes and multi-byte characters that the range suite does
//! not.
//!
//! Kept in one place because the assembly logic is the load-bearing part. A
//! change to how continuations, blank lines or tabs are interleaved would
//! otherwise land in one suite and silently stop reaching the other, leaving
//! both green while testing different bodies.

use proptest::prelude::*;

/// Builds a strategy producing one source fragment from `alphabet`.
///
/// A fragment is one to three pieces concatenated, so a body can contain
/// adjacent braces, escaped quotes or multi-byte characters in any order.
fn source_piece(alphabet: &'static [&'static str]) -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(alphabet), 1..4).prop_map(|pieces| pieces.concat())
}

/// Appends one continuation and the whitespace it consumes to `body`.
///
/// `whitespace` is `(spaces, blank_lines, tabs)`. A blank line inside the
/// consumed run is what separates a single-line continuation from one that
/// spans several, so generating all three axes independently is what lets the
/// suites reach both.
pub(crate) fn append_continuation(body: &mut String, whitespace: (u8, u8, u8)) {
    let (spaces, blank_lines, tabs) = whitespace;
    body.push('\\');
    body.push('\n');
    for _ in 0..spaces {
        body.push(' ');
    }
    for _ in 0..blank_lines {
        body.push('\n');
    }
    for _ in 0..tabs {
        body.push('\t');
    }
}

/// Builds a strategy producing a body with `2..=6` fragments and its join
/// count.
///
/// The count is always one fewer than the fragment count, because a
/// continuation sits between each pair and never at either end.
pub(crate) fn body_with_continuations(
    alphabet: &'static [&'static str],
) -> impl Strategy<Value = (String, usize)> {
    prop::collection::vec(source_piece(alphabet), 2..=6).prop_flat_map(|source_fragments| {
        let join_count = source_fragments.len().saturating_sub(1);
        (
            Just(source_fragments),
            prop::collection::vec((1_u8..4, 0_u8..3, 0_u8..3), join_count),
        )
            .prop_map(|(body_fragments, whitespace)| {
                let mut body = String::new();
                for (index, fragment) in body_fragments.iter().enumerate() {
                    body.push_str(fragment);
                    whitespace
                        .get(index)
                        .copied()
                        .into_iter()
                        .for_each(|settings| append_continuation(&mut body, settings));
                }
                (body, body_fragments.len().saturating_sub(1))
            })
    })
}
