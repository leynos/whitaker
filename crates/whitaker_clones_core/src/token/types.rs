//! Core token-pass types shared by normalization and fingerprinting.

use std::{fmt, num::NonZeroUsize, ops::Range};

use super::error::TokenPassError;

/// Normalization profile for the token pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NormProfile {
    /// Type-1 profile: strip trivia but preserve identifier and literal text.
    #[default]
    T1,
    /// Type-2 profile: canonicalize identifiers, lifetimes, and literals.
    T2,
}

/// A single normalized token with its original byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedToken {
    /// The normalized semantic kind.
    pub kind: NormalizedTokenKind,
    /// The original byte range in the source text.
    pub range: Range<usize>,
}

impl NormalizedToken {
    /// Creates a normalized token from its kind and original source range.
    #[must_use]
    pub const fn new(kind: NormalizedTokenKind, range: Range<usize>) -> Self {
        Self { kind, range }
    }
}

/// The normalized representation of a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NormalizedTokenKind {
    /// A fixed keyword or punctuation atom.
    Atom(&'static str),
    /// An identifier or raw identifier.
    Identifier(IdentifierSymbol),
    /// A lifetime parameter or label.
    Lifetime(IdentifierSymbol),
    /// A literal value.
    Literal(LiteralSymbol),
}

impl fmt::Display for NormalizedTokenKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Atom(atom) => formatter.write_str(atom),
            Self::Identifier(symbol) | Self::Lifetime(symbol) => {
                fmt::Display::fmt(symbol, formatter)
            }
            Self::Literal(symbol) => fmt::Display::fmt(symbol, formatter),
        }
    }
}

/// Identifier representation for `T1` and `T2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentifierSymbol {
    /// Preserve the original identifier or lifetime text.
    Original(String),
    /// Canonicalize an identifier or lifetime by encounter order.
    Canonical(usize),
}

impl fmt::Display for IdentifierSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Original(value) => formatter.write_str(value),
            Self::Canonical(index) => write!(formatter, "<ID_{index}>"),
        }
    }
}

/// Literal representation for `T1` and `T2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiteralSymbol {
    /// Preserve the original literal source text.
    Original(String),
    /// Canonicalize the literal to a category marker.
    Canonical(&'static str),
}

impl fmt::Display for LiteralSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Original(value) => formatter.write_str(value),
            Self::Canonical(value) => formatter.write_str(value),
        }
    }
}

/// A validated `k` value for shingling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShingleSize(NonZeroUsize);

impl ShingleSize {
    /// Returns the validated `k` as a plain `usize`.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

impl TryFrom<usize> for ShingleSize {
    type Error = TokenPassError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match NonZeroUsize::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(TokenPassError::ZeroShingleSize),
        }
    }
}

/// A validated winnowing window size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WinnowWindow(NonZeroUsize);

impl WinnowWindow {
    /// Returns the validated window size as a plain `usize`.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

impl TryFrom<usize> for WinnowWindow {
    type Error = TokenPassError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match NonZeroUsize::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(TokenPassError::ZeroWinnowWindow),
        }
    }
}

/// A retained fingerprint and its source range.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fingerprint {
    /// The 64-bit Rabin-Karp or winnowed hash value.
    pub hash: u64,
    /// The byte range spanned by the contributing tokens.
    pub range: Range<usize>,
}

impl Fingerprint {
    /// Creates a fingerprint from a hash value and source range.
    #[must_use]
    pub const fn new(hash: u64, range: Range<usize>) -> Self {
        Self { hash, range }
    }
}
