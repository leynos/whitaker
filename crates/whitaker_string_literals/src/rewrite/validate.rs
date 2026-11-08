//! Re-lexes a rendered replacement to prove it is one `concat!` invocation.
//!
//! The rewrite builder works on byte offsets and string fragments, so nothing
//! in its own arithmetic can tell it whether the text it emitted actually
//! parses. This module answers that separately, with `rustc_lexer` alone: the
//! rendered text must tokenize as `concat ! (` followed by at least two
//! unsuffixed, terminated string literals separated by commas and closed by a
//! single `)`, with nothing left over.

use rustc_lexer::{LiteralKind, Token, TokenKind, tokenize};

/// Reports whether a rendered replacement parses back as a `concat!` call.
///
/// This is the last gate before a suggestion is offered: a replacement that
/// does not re-lex as one `concat!` invocation with at least two literal
/// arguments would produce code that does not compile.
pub(super) fn is_concat_macro_expression(source: &str) -> bool {
    let mut tokens = tokenize(source).filter(|token| token.kind != TokenKind::Whitespace);
    if !has_concat_macro_head(source, &mut tokens) {
        return false;
    }
    if !has_concat_macro_arguments(&mut tokens) {
        return false;
    }

    tokens.next().is_none()
}

/// Consumes the `concat ! (` head, or reports that it is absent.
///
/// The ident check compares the token's own source slice, so a whitespace or
/// comment token between `concat` and `!` is skipped by the caller's filter
/// without letting `concat` be matched as some longer name.
fn has_concat_macro_head(source: &str, tokens: &mut impl Iterator<Item = Token>) -> bool {
    let Some(ident) = tokens.next() else {
        return false;
    };
    let Some(ident_source) = source.get(..ident.len) else {
        return false;
    };
    if ident.kind != TokenKind::Ident || ident_source != "concat" {
        return false;
    }
    if !matches!(tokens.next().map(|token| token.kind), Some(TokenKind::Not))
        || !matches!(
            tokens.next().map(|token| token.kind),
            Some(TokenKind::OpenParen)
        )
    {
        return false;
    }

    true
}

/// Consumes comma-separated unsuffixed string literals up to the closing paren.
///
/// The small state machine is the point: `expects_literal` and `can_close`
/// together reject the shapes that would tokenize cleanly but not compile as
/// `concat!` arguments, such as `concat!("a",)` or `concat!("a" "b")`.
fn has_concat_macro_arguments(tokens: &mut impl Iterator<Item = Token>) -> bool {
    let mut literal_count = 0_usize;
    let mut expects_literal = true;
    let mut can_close = false;
    loop {
        let Some(token) = tokens.next() else {
            return false;
        };

        match token.kind {
            TokenKind::Literal {
                kind: LiteralKind::Str { terminated: true },
                suffix_start,
            } if expects_literal && suffix_start == token.len => {
                literal_count = literal_count.saturating_add(1);
                expects_literal = false;
                can_close = true;
            }
            TokenKind::Comma if !expects_literal => expects_literal = true,
            TokenKind::CloseParen if can_close => break,
            _ => return false,
        }
    }

    literal_count >= 2
}
