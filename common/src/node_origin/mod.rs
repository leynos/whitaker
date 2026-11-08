//! Compiler-free classification of HIR node provenance.
//!
//! Complexity-sensitive lints need to distinguish code a person wrote from
//! code a macro expansion or compiler pass produced. This module owns that
//! decision as a pure function over expansion marks, so the policy can be
//! enumerated and proved without linking rustc. The rustc adapter that turns a
//! syntax context into marks lives in the `whitaker` crate behind the
//! `dylint-driver` feature.

#[cfg(test)]
mod tests;

/// Where a HIR node came from.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NodeOrigin {
    /// Written by a person in the crate source, possibly lowered by rustc.
    Authored,
    /// Produced by a macro expansion or a compiler AST pass.
    Generated,
}

impl NodeOrigin {
    /// Returns `true` when the node was generated.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_common::node_origin::NodeOrigin;
    ///
    /// assert!(!NodeOrigin::Authored.is_generated());
    /// assert!(NodeOrigin::Generated.is_generated());
    /// ```
    #[must_use]
    pub const fn is_generated(self) -> bool { matches!(self, Self::Generated) }
}

/// One expansion mark on a syntax context, outermost first.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExpansionMark {
    /// A compiler desugaring, such as `?`, `for`, or `async`.
    Desugaring,
    /// A `macro_rules!`, attribute, or derive expansion.
    Macro,
    /// A compiler AST pass, such as standard-import injection.
    AstPass,
}

/// Classifies a node from its expansion marks, outermost first.
///
/// Desugarings are transparent: rustc lowers source constructs such as `for`
/// and `?` into marks, and those marks must not turn authored code into
/// generated code. The first macro or AST-pass mark proves the node came from
/// generated code. A sequence containing only desugarings, including the empty
/// sequence, is authored.
///
/// # Examples
///
/// ```
/// use whitaker_common::node_origin::{ExpansionMark, NodeOrigin, classify};
///
/// assert_eq!(classify([]), NodeOrigin::Authored);
/// assert_eq!(
///     classify([ExpansionMark::Desugaring, ExpansionMark::Macro]),
///     NodeOrigin::Generated,
/// );
/// ```
#[must_use]
pub fn classify(marks: impl IntoIterator<Item = ExpansionMark>) -> NodeOrigin {
    for mark in marks {
        match mark {
            ExpansionMark::Desugaring => {}
            ExpansionMark::Macro | ExpansionMark::AstPass => {
                return NodeOrigin::Generated;
            }
        }
    }
    NodeOrigin::Authored
}
