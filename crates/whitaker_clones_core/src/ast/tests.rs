//! Tests for parser-independent AST feature extraction.

use super::{
    AstResult, ByteSpan, Depth, KindId, KindWeight, LeafClass, NormalisedNode, NormalisedTree,
    Production, canonical_hash, kind_counts, kind_histogram, production_multiset,
};
use rstest::rstest;

#[rstest]
fn kind_counts_record_depth_resolved_counts() -> AstResult<()> {
    let counts = kind_counts(&feature_tree()?);

    assert_eq!(counts.count(KindId::new(1), Depth::root()), 1);
    assert_eq!(counts.count(KindId::new(2), Depth::new(1)), 1);
    assert_eq!(counts.count(KindId::new(2), Depth::new(2)), 1);
    assert_eq!(counts.count(KindId::new(4), Depth::new(2)), 1);

    Ok(())
}

#[rstest]
fn weighted_histogram_applies_dyadic_depth_weights() -> AstResult<()> {
    let histogram = kind_histogram(&feature_tree()?);

    assert_eq!(
        histogram.get(KindId::new(1)).map(KindWeight::get),
        Some(KindWeight::SCALE)
    );
    assert_eq!(
        histogram.get(KindId::new(2)).map(KindWeight::get),
        Some((KindWeight::SCALE >> 1) + (KindWeight::SCALE >> 2))
    );
    assert_eq!(
        histogram.get(KindId::new(4)).map(KindWeight::get),
        Some(KindWeight::SCALE >> 2)
    );

    Ok(())
}

#[rstest]
fn production_multiset_records_bigrams_and_trigrams() -> AstResult<()> {
    let productions = production_multiset(&feature_tree()?);

    assert_eq!(
        productions.count(Production::Bigram(KindId::new(1), KindId::new(2))),
        1
    );
    assert_eq!(
        productions.count(Production::Bigram(KindId::new(3), KindId::new(4))),
        1
    );
    assert_eq!(
        productions.count(Production::Trigram(
            KindId::new(1),
            KindId::new(3),
            KindId::new(2)
        )),
        1
    );
    assert_eq!(
        productions.count(Production::Trigram(
            KindId::new(1),
            KindId::new(2),
            KindId::new(4)
        )),
        0
    );

    Ok(())
}

#[rstest]
fn canonical_hash_is_stable_for_equivalent_trees() -> AstResult<()> {
    assert_eq!(
        canonical_hash(&feature_tree()?),
        canonical_hash(&feature_tree()?)
    );

    Ok(())
}

#[rstest]
fn canonical_hash_is_sensitive_to_child_order() -> AstResult<()> {
    assert_ne!(
        canonical_hash(&feature_tree()?),
        canonical_hash(&reordered_tree()?)
    );

    Ok(())
}

#[rstest]
fn canonical_hash_is_sensitive_to_leaf_class() -> AstResult<()> {
    assert_ne!(
        canonical_hash(&feature_tree()?),
        canonical_hash(&different_leaf_tree()?)
    );

    Ok(())
}

fn feature_tree() -> AstResult<NormalisedTree> {
    tree_with_root(NormalisedNode::new(
        KindId::new(1),
        None,
        vec![ident(KindId::new(2)), branch()],
    ))
}

fn reordered_tree() -> AstResult<NormalisedTree> {
    tree_with_root(NormalisedNode::new(
        KindId::new(1),
        None,
        vec![branch(), ident(KindId::new(2))],
    ))
}

fn different_leaf_tree() -> AstResult<NormalisedTree> {
    tree_with_root(NormalisedNode::new(
        KindId::new(1),
        None,
        vec![literal(KindId::new(2)), branch()],
    ))
}

fn tree_with_root(root: NormalisedNode) -> AstResult<NormalisedTree> {
    Ok(NormalisedTree::new(root, ByteSpan::new("fn f() {}", 0, 2)?))
}

fn branch() -> NormalisedNode {
    NormalisedNode::new(
        KindId::new(3),
        None,
        vec![ident(KindId::new(2)), literal(KindId::new(4))],
    )
}

fn ident(kind: KindId) -> NormalisedNode {
    NormalisedNode::new(kind, Some(LeafClass::Ident), Vec::new())
}

fn literal(kind: KindId) -> NormalisedNode {
    NormalisedNode::new(kind, Some(LeafClass::Literal), Vec::new())
}
