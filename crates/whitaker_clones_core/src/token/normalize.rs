//! `rustc_lexer`-based normalization for clone-detection token streams.

use std::{collections::BTreeMap, ops::Range};

use rustc_lexer::{LiteralKind, TokenKind, strip_shebang, tokenize};

use super::{
    Result,
    error::TokenPassError,
    types::{IdentifierSymbol, LiteralSymbol, NormProfile, NormalizedToken, NormalizedTokenKind},
};

const KEYWORDS: &[&str] = &[
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

#[derive(Default)]
struct CanonicalState {
    identifiers: BTreeMap<String, usize>,
    lifetimes: BTreeMap<String, usize>,
}

impl CanonicalState {
    fn identifier_index(&mut self, text: &str) -> usize {
        let next = self.identifiers.len();
        *self.identifiers.entry(text.to_owned()).or_insert(next)
    }

    fn lifetime_index(&mut self, text: &str) -> usize {
        let next = self.lifetimes.len();
        *self.lifetimes.entry(text.to_owned()).or_insert(next)
    }
}

/// Normalizes Rust source for the requested clone-detection profile.
///
/// The returned tokens retain their original byte ranges so later stages can
/// map fingerprints back to source regions.
///
/// # Errors
///
/// Returns [`TokenPassError`] if the lexer encounters an unsupported token or
/// an unterminated block comment or literal.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{NormProfile, normalize};
///
/// let tokens = normalize("fn demo(x: i32) { x + 1 }", NormProfile::T2)?;
/// let labels = tokens
///     .iter()
///     .map(|token| token.kind.to_string())
///     .collect::<Vec<_>>();
///
/// assert_eq!(
///     labels,
///     vec!["fn", "<ID_0>", "(", "<ID_1>", ":", "<ID_2>", ")", "{", "<ID_1>", "+", "<NUM>", "}"]
/// );
/// # Ok::<(), whitaker_clones_core::TokenPassError>(())
/// ```
pub fn normalize(source: &str, profile: NormProfile) -> Result<Vec<NormalizedToken>> {
    let shebang_len = strip_shebang(source).unwrap_or(0);
    let mut state = CanonicalState::default();
    let mut normalized = Vec::new();
    let mut offset = shebang_len;
    let body = match source.get(shebang_len..) {
        Some(body) => body,
        None => source,
    };

    for token in tokenize(body) {
        let end = offset + token.len;
        let range = offset..end;
        let text = source.get(range.clone()).ok_or({
            TokenPassError::UnsupportedToken {
                start: range.start,
                end: range.end,
            }
        })?;

        match token.kind {
            TokenKind::Whitespace | TokenKind::LineComment => {}
            TokenKind::BlockComment { terminated } => {
                if !terminated {
                    return Err(TokenPassError::UnterminatedBlockComment {
                        start: range.start,
                        end: range.end,
                    });
                }
            }
            TokenKind::Unknown => {
                return Err(TokenPassError::UnsupportedToken {
                    start: range.start,
                    end: range.end,
                });
            }
            TokenKind::Literal { kind, .. } => {
                ensure_literal_is_terminated(kind, &range)?;
                normalized.push(NormalizedToken::new(
                    normalize_literal(text, kind, profile),
                    range.clone(),
                ));
            }
            TokenKind::Ident => normalized.push(NormalizedToken::new(
                normalize_ident(text, profile, &mut state),
                range.clone(),
            )),
            TokenKind::RawIdent => normalized.push(NormalizedToken::new(
                normalize_identifier_text(text, profile, &mut state),
                range.clone(),
            )),
            TokenKind::Lifetime { .. } => normalized.push(NormalizedToken::new(
                normalize_lifetime(text, profile, &mut state),
                range.clone(),
            )),
            other => normalized.push(NormalizedToken::new(
                NormalizedTokenKind::Atom(atom_label(other)),
                range.clone(),
            )),
        }

        offset = end;
    }

    Ok(normalized)
}

fn ensure_literal_is_terminated(kind: LiteralKind, range: &Range<usize>) -> Result<()> {
    let terminated = match kind {
        LiteralKind::Int { .. } | LiteralKind::Float { .. } => true,
        LiteralKind::Char { terminated }
        | LiteralKind::Byte { terminated }
        | LiteralKind::Str { terminated }
        | LiteralKind::ByteStr { terminated } => terminated,
        LiteralKind::RawStr { terminated, .. } | LiteralKind::RawByteStr { terminated, .. } => {
            terminated
        }
    };

    if terminated {
        Ok(())
    } else {
        Err(TokenPassError::UnterminatedLiteral {
            literal_kind: literal_kind_label(kind),
            start: range.start,
            end: range.end,
        })
    }
}

fn normalize_ident(
    text: &str,
    profile: NormProfile,
    state: &mut CanonicalState,
) -> NormalizedTokenKind {
    match keyword_label(text) {
        Some(keyword) => NormalizedTokenKind::Atom(keyword),
        None => normalize_identifier_text(text, profile, state),
    }
}

fn normalize_identifier_text(
    text: &str,
    profile: NormProfile,
    state: &mut CanonicalState,
) -> NormalizedTokenKind {
    match profile {
        NormProfile::T1 => {
            NormalizedTokenKind::Identifier(IdentifierSymbol::Original(text.to_owned()))
        }
        NormProfile::T2 => NormalizedTokenKind::Identifier(IdentifierSymbol::Canonical(
            state.identifier_index(text),
        )),
    }
}

fn normalize_lifetime(
    text: &str,
    profile: NormProfile,
    state: &mut CanonicalState,
) -> NormalizedTokenKind {
    match profile {
        NormProfile::T1 => {
            NormalizedTokenKind::Lifetime(IdentifierSymbol::Original(text.to_owned()))
        }
        NormProfile::T2 => {
            NormalizedTokenKind::Lifetime(IdentifierSymbol::Canonical(state.lifetime_index(text)))
        }
    }
}

fn normalize_literal(text: &str, kind: LiteralKind, profile: NormProfile) -> NormalizedTokenKind {
    match profile {
        NormProfile::T1 => NormalizedTokenKind::Literal(LiteralSymbol::Original(text.to_owned())),
        NormProfile::T2 => {
            NormalizedTokenKind::Literal(LiteralSymbol::Canonical(canonical_literal_label(kind)))
        }
    }
}

fn keyword_label(text: &str) -> Option<&'static str> {
    KEYWORDS.iter().copied().find(|keyword| *keyword == text)
}

fn canonical_literal_label(kind: LiteralKind) -> &'static str {
    match kind {
        LiteralKind::Int { .. } | LiteralKind::Float { .. } => "<NUM>",
        LiteralKind::Char { .. } => "<CHAR>",
        LiteralKind::Byte { .. } => "<BYTE>",
        LiteralKind::Str { .. } | LiteralKind::RawStr { .. } => "<STR>",
        LiteralKind::ByteStr { .. } | LiteralKind::RawByteStr { .. } => "<BYTE_STR>",
    }
}

fn literal_kind_label(kind: LiteralKind) -> &'static str {
    match kind {
        LiteralKind::Int { .. } => "integer",
        LiteralKind::Float { .. } => "float",
        LiteralKind::Char { .. } => "character",
        LiteralKind::Byte { .. } => "byte",
        LiteralKind::Str { .. } | LiteralKind::RawStr { .. } => "string",
        LiteralKind::ByteStr { .. } | LiteralKind::RawByteStr { .. } => "byte string",
    }
}

fn atom_label(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Semi => ";",
        TokenKind::Comma => ",",
        TokenKind::Dot => ".",
        TokenKind::OpenParen => "(",
        TokenKind::CloseParen => ")",
        TokenKind::OpenBrace => "{",
        TokenKind::CloseBrace => "}",
        TokenKind::OpenBracket => "[",
        TokenKind::CloseBracket => "]",
        TokenKind::At => "@",
        TokenKind::Pound => "#",
        TokenKind::Tilde => "~",
        TokenKind::Question => "?",
        TokenKind::Colon => ":",
        TokenKind::Dollar => "$",
        TokenKind::Eq => "=",
        TokenKind::Not => "!",
        TokenKind::Lt => "<",
        TokenKind::Gt => ">",
        TokenKind::Minus => "-",
        TokenKind::And => "&",
        TokenKind::Or => "|",
        TokenKind::Plus => "+",
        TokenKind::Star => "*",
        TokenKind::Slash => "/",
        TokenKind::Caret => "^",
        TokenKind::Percent => "%",
        _ => "<UNREACHABLE>",
    }
}
