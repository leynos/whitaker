//! Tests for the AST feature-extraction skeleton.

use super::{ByteSpan, KindId, NormalisedNode, NormalisedTree, canonical_hash};

#[test]
fn canonical_hash_returns_neutral_skeleton_value() -> Result<(), crate::AstError> {
    let span = ByteSpan::new("fn f() {}", 0, 2)?;
    let tree = NormalisedTree::new(NormalisedNode::new(KindId::new(1), None, Vec::new()), span);

    assert_eq!(canonical_hash(&tree).to_hex(), "0000000000000000");

    Ok(())
}
