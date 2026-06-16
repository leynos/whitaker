//! Canonical AST subtree hashing over lowered trees.

use std::fmt;

use super::{LeafClass, NormalisedNode, NormalisedTree};
use crate::hashing::{
    FNV_OFFSET_BASIS, PARSER_SCHEMA_VERSION, mix_byte, mix_bytes, mix_u16, mix_u64,
};

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
/// assert_eq!(canonical_hash(&tree).to_hex().len(), 16);
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
#[must_use]
pub fn canonical_hash(tree: &NormalisedTree) -> AstHash {
    AstHash(hash_node(seed_hash(), tree.root()))
}

fn seed_hash() -> u64 {
    mix_bytes(FNV_OFFSET_BASIS, PARSER_SCHEMA_VERSION.as_bytes())
}

fn hash_node(mut hash: u64, node: &NormalisedNode) -> u64 {
    hash = mix_byte(hash, b'n');
    hash = mix_u16(hash, node.kind().get());
    hash = mix_byte(hash, leaf_tag(node.leaf()));
    hash = mix_u64(hash, child_count(node));
    for child in node.children() {
        hash = mix_u64(hash, hash_node(seed_hash(), child));
    }
    hash
}

fn child_count(node: &NormalisedNode) -> u64 {
    u64::try_from(node.children().len()).unwrap_or(u64::MAX)
}

fn leaf_tag(leaf: Option<LeafClass>) -> u8 {
    match leaf {
        Some(LeafClass::Ident) => b'i',
        Some(LeafClass::Literal) => b'l',
        Some(LeafClass::Other) => b'o',
        None => b'n',
    }
}
