//! `rustc_lexer`-based normalization for clone-detection token streams.

use std::{collections::BTreeMap, ops::Range};

use rustc_lexer::{LiteralKind, TokenKind, strip_shebang, tokenize};

use super::{
    Result,
    error::TokenPassError,
    types::{IdentifierSymbol, LiteralSymbol, NormProfile, NormalizedToken, NormalizedTokenKind},
};

const KEYWORDS: &[&str] = &[
    "Self",
    "abstract",
    "as",
    "async",
    "await",
    "become",
    "box",
    "break",
    "const",
    "continue",
    "crate",
    "do",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "final",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "macro",
    "match",
    "mod",
    "move",
    "mut",
    "override",
    "priv",
    "pub",
    "ref",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "try",
    "type",
    "typeof",
    "union",
    "unsafe",
    "unsized",
    "use",
    "virtual",
    "where",
    "while",
    "yield",
    "macro_rules",
    "raw",
    "safe",
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

struct TokenInput<'src> {
    kind: TokenKind,
    text: &'src str,
    range: std::ops::Range<usize>,
    profile: NormProfile,
}

fn process_token(
    input: TokenInput<'_>,
    state: &mut CanonicalState,
) -> Result<Option<NormalizedToken>> {
    let TokenInput {
        kind,
        text,
        range,
        profile,
    } = input;

    match kind {
        TokenKind::Whitespace | TokenKind::LineComment => Ok(None),
        TokenKind::BlockComment { terminated } => {
            if terminated {
                Ok(None)
            } else {
                Err(TokenPassError::UnterminatedBlockComment {
                    start: range.start,
                    end: range.end,
                })
            }
        }
        TokenKind::Unknown => Err(TokenPassError::UnsupportedToken {
            start: range.start,
            end: range.end,
        }),
        TokenKind::Literal { kind, .. } => {
            ensure_literal_is_terminated(kind, &range)?;
            Ok(Some(NormalizedToken::new(
                normalize_literal(text, kind, profile),
                range,
            )))
        }
        TokenKind::Ident => Ok(Some(NormalizedToken::new(
            normalize_ident(text, profile, state),
            range,
        ))),
        TokenKind::RawIdent => Ok(Some(NormalizedToken::new(
            normalize_symbolic_text(
                raw_identifier_text(text),
                profile,
                || state.identifier_index(raw_identifier_text(text)),
                NormalizedTokenKind::Identifier,
            ),
            range,
        ))),
        TokenKind::Lifetime { .. } => Ok(Some(NormalizedToken::new(
            normalize_symbolic_text(
                text,
                profile,
                || state.lifetime_index(text),
                NormalizedTokenKind::Lifetime,
            ),
            range,
        ))),
        other => Ok(Some(NormalizedToken::new(
            NormalizedTokenKind::Atom(atom_label(other)),
            range,
        ))),
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
    let body = source.get(shebang_len..).unwrap_or(source);

    for token in tokenize(body) {
        let end = offset + token.len;
        let range = offset..end;
        let text = source
            .get(range.clone())
            .ok_or(TokenPassError::UnsupportedToken {
                start: range.start,
                end: range.end,
            })?;

        let input = TokenInput {
            kind: token.kind,
            text,
            range,
            profile,
        };
        if let Some(tok) = process_token(input, &mut state)? {
            normalized.push(tok);
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
            literal_kind: literal_labels(kind).kind,
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
        None => normalize_symbolic_text(
            text,
            profile,
            || state.identifier_index(text),
            NormalizedTokenKind::Identifier,
        ),
    }
}

fn normalize_literal(text: &str, kind: LiteralKind, profile: NormProfile) -> NormalizedTokenKind {
    match profile {
        NormProfile::T1 => NormalizedTokenKind::Literal(LiteralSymbol::Original(text.to_owned())),
        NormProfile::T2 => {
            NormalizedTokenKind::Literal(LiteralSymbol::Canonical(literal_labels(kind).canonical))
        }
    }
}

/// A flat slice keeps keyword lookup dependency-free; 57 entries are cheap to
/// scan and avoid pulling in a perfect-hash crate for an unprofiled path.
fn keyword_label(text: &str) -> Option<&'static str> {
    KEYWORDS.iter().copied().find(|keyword| *keyword == text)
}

struct LiteralLabels {
    canonical: &'static str,
    kind: &'static str,
}

fn literal_labels(kind: LiteralKind) -> LiteralLabels {
    match kind {
        LiteralKind::Int { .. } => LiteralLabels {
            canonical: "<NUM>",
            kind: "integer",
        },
        LiteralKind::Float { .. } => LiteralLabels {
            canonical: "<NUM>",
            kind: "float",
        },
        LiteralKind::Char { .. } => LiteralLabels {
            canonical: "<CHAR>",
            kind: "character",
        },
        LiteralKind::Byte { .. } => LiteralLabels {
            canonical: "<BYTE>",
            kind: "byte",
        },
        LiteralKind::Str { .. } | LiteralKind::RawStr { .. } => LiteralLabels {
            canonical: "<STR>",
            kind: "string",
        },
        LiteralKind::ByteStr { .. } | LiteralKind::RawByteStr { .. } => LiteralLabels {
            canonical: "<BYTE_STR>",
            kind: "byte string",
        },
    }
}

fn normalize_symbolic_text(
    text: &str,
    profile: NormProfile,
    canonical_index: impl FnOnce() -> usize,
    wrap: fn(IdentifierSymbol) -> NormalizedTokenKind,
) -> NormalizedTokenKind {
    let symbol = match profile {
        NormProfile::T1 => IdentifierSymbol::Original(text.to_owned()),
        NormProfile::T2 => IdentifierSymbol::Canonical(canonical_index()),
    };
    wrap(symbol)
}

fn raw_identifier_text(text: &str) -> &str {
    text.strip_prefix("r#").unwrap_or(text)
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
        TokenKind::Whitespace
        | TokenKind::LineComment
        | TokenKind::BlockComment { .. }
        | TokenKind::Ident
        | TokenKind::RawIdent
        | TokenKind::Literal { .. }
        | TokenKind::Lifetime { .. }
        | TokenKind::Unknown => {
            debug_assert!(
                false,
                "Token kind {:?} should be handled before atom_label",
                kind
            );
            "<UNREACHABLE>"
        }
    }
}
