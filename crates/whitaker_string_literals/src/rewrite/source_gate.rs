//! Re-lexes a candidate literal and join to prove the split is well-formed.
//!
//! `ConcatRewrite::build` trusts the offsets it is handed only as far as it
//! can check them. This module holds the three predicates that do the
//! checking, each written directly against the source bytes so a caller-built
//! `Continuation` cannot smuggle in a range the scanner would never produce.

use rustc_lexer::{LiteralKind, TokenKind, tokenize};

use crate::continuation::{Continuation, ContinuationKind};

/// Reports whether `source` is exactly one terminated, unsuffixed string.
///
/// The length comparison is what rejects trailing tokens: tokenizing happily
/// stops after a prefix, so a source of `"a" + "b"` would otherwise pass on
/// its first token alone.
pub(super) fn is_single_cooked_string_token(source: &str) -> bool {
    let mut tokens = tokenize(source);
    let Some(token) = tokens.next() else {
        return false;
    };
    let TokenKind::Literal {
        kind: LiteralKind::Str { terminated: true },
        suffix_start,
    } = token.kind
    else {
        return false;
    };

    token.len == source.len() && suffix_start == token.len && tokens.next().is_none()
}

/// Returns whether `join` may be split out of `source` at this position.
///
/// Four conditions must hold together, and each is a separate defence against
/// a caller passing continuations that the scanner would not itself produce:
/// the join must be classified as an interior join, it must be one the body's
/// own scan reported, it must not start before the previous split ended, and
/// its byte ranges must spell exactly one unpaired backslash and LF followed
/// by consumed whitespace. Folding them into one predicate keeps the builder's
/// loop reading as "validate, then split" rather than as four early returns.
pub(super) fn is_selected_join(
    source: &str,
    join: &Continuation,
    scanned_joins: &[Continuation],
    previous_end: usize,
) -> bool {
    join.kind == ContinuationKind::Join
        && scanned_joins.contains(join)
        && join.escape_range.start.get() >= previous_end
        && is_valid_join_range(source, join)
}

/// Reports whether `join`'s byte ranges spell a well-formed continuation.
///
/// Both ranges are looked up with `get`, so an out-of-bounds range from a
/// caller-built `Continuation` returns `false` instead of panicking.
fn is_valid_join_range(source: &str, join: &Continuation) -> bool {
    let escape_start = join.escape_range.start.get();
    let escape_end = join.escape_range.end.get();
    let skipped_start = join.skipped_whitespace_range.start.get();
    let skipped_end = join.skipped_whitespace_range.end.get();
    let Some(escape) = source.as_bytes().get(escape_start..escape_end) else {
        return false;
    };
    let Some(skipped) = source.as_bytes().get(skipped_start..skipped_end) else {
        return false;
    };

    escape == b"\\\n"
        && escape_end == skipped_start
        && skipped
            .iter()
            .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
}
