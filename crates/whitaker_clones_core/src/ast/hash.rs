//! Canonical AST subtree hashing over lowered trees.

use std::fmt;

use super::NormalisedTree;

/// Opaque canonical AST subtree hash.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::ast::{ByteSpan, KindId, NormalisedNode, NormalisedTree};
/// use whitaker_clones_core::canonical_hash;
///
/// let span = ByteSpan::new("fn f() {}", 0, 2)?;
/// let tree = NormalisedTree::new(NormalisedNode::new(KindId::new(1), None, Vec::new()), span);
/// assert_eq!(canonical_hash(&tree).to_hex(), "0000000000000000");
/// # Ok::<(), whitaker_clones_core::AstError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AstHash(u64);

impl AstHash {
    /// Renders the hash as a fixed-width lowercase hexadecimal string.
    #[must_use]
    pub fn to_hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

impl fmt::Display for AstHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// Computes the canonical hash for `tree`.
///
/// Stage C replaces the neutral skeleton value with Merkle-style folding.
#[must_use]
pub fn canonical_hash(_tree: &NormalisedTree) -> AstHash {
    AstHash(0)
}
