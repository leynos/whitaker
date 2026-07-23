//! Tests for parser-independent AST feature extraction.

use super::{
    AstResult, ByteSpan, Depth, KindId, KindWeight, LeafClass, NormalizedNode, NormalizedTree,
    Production, canonical_hash, kind_counts, kind_histogram, production_multiset,
    select_smallest_covering, weighted_histogram,
};
use proptest::prelude::*;
use rstest::rstest;

#[cfg(not(feature = "parser"))]
#[rstest]
fn parser_free_lowering_reports_parser_unavailable() -> AstResult<()> {
    let source = "fn f() {}";
    let span = ByteSpan::new(source, 0, source.len() as u32)?;

    assert_eq!(
        super::lower_span(source, span),
        Err(super::AstError::ParserUnavailable)
    );
    Ok(())
}

#[rstest]
fn malformed_covering_candidates_are_ignored() {
    let malformed_start = 8;
    let malformed_end = 3;
    let candidates = [malformed_start..malformed_end, 0..10];

    assert_eq!(select_smallest_covering(&candidates, &(4..5)), Some(1));
}

#[rstest]
fn reversed_target_ranges_are_rejected() {
    let target_start = 5;
    let target_end = 3;
    let candidate = 0..10;

    assert_eq!(
        select_smallest_covering(
            std::slice::from_ref(&candidate),
            &(target_start..target_end)
        ),
        None
    );
    assert_eq!(
        select_smallest_covering(std::slice::from_ref(&candidate), &(3..5)),
        Some(0)
    );
}

#[rstest]
fn equal_width_covering_candidates_select_the_first() {
    let candidates = [0..5, 0..5];

    assert_eq!(select_smallest_covering(&candidates, &(2..3)), Some(0));
}

#[rstest]
fn deeply_nested_trees_extract_features_and_hash_without_recursion() -> AstResult<()> {
    let tree = tree_with_root(deep_chain(2_048))?;

    assert_eq!(kind_counts(&tree).iter().count(), 2_049);
    assert_eq!(production_multiset(&tree).bigrams().count(), 1);
    assert_eq!(canonical_hash(&tree).to_hex().len(), 16);
    Ok(())
}

#[rstest]
fn kind_counts_record_depth_resolved_counts() -> AstResult<()> {
    let counts = kind_counts(&feature_tree()?);

    let expected = [
        (KindId::new(1), Depth::root(), 1),
        (KindId::new(2), Depth::new(1), 1),
        (KindId::new(2), Depth::new(2), 1),
        (KindId::new(3), Depth::new(1), 1),
        (KindId::new(4), Depth::new(2), 1),
    ];

    assert_eq!(counts.iter().collect::<Vec<_>>(), expected);

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
fn weighted_histogram_accumulates_four_equal_depth_one_kinds() -> AstResult<()> {
    let kind = KindId::new(9);
    let tree = tree_with_root(NormalizedNode::new(
        KindId::new(1),
        None,
        (0..4).map(|_| ident(kind)).collect(),
    ))?;

    assert_eq!(
        kind_histogram(&tree).get(kind).map(KindWeight::get),
        Some(4 * (KindWeight::SCALE >> 1))
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

fn feature_tree() -> AstResult<NormalizedTree> {
    tree_with_root(NormalizedNode::new(
        KindId::new(1),
        None,
        vec![ident(KindId::new(2)), branch()],
    ))
}

fn reordered_tree() -> AstResult<NormalizedTree> {
    tree_with_root(NormalizedNode::new(
        KindId::new(1),
        None,
        vec![branch(), ident(KindId::new(2))],
    ))
}

fn different_leaf_tree() -> AstResult<NormalizedTree> {
    tree_with_root(NormalizedNode::new(
        KindId::new(1),
        None,
        vec![literal(KindId::new(2)), branch()],
    ))
}

fn tree_with_root(root: NormalizedNode) -> AstResult<NormalizedTree> {
    Ok(NormalizedTree::new(root, ByteSpan::new("fn f() {}", 0, 2)?))
}

fn deep_chain(depth: usize) -> NormalizedNode {
    (0..depth).fold(
        NormalizedNode::new(KindId::new(2), None, Vec::new()),
        |child, _| NormalizedNode::new(KindId::new(2), None, vec![child]),
    )
}

fn branch() -> NormalizedNode {
    NormalizedNode::new(
        KindId::new(3),
        None,
        vec![ident(KindId::new(2)), literal(KindId::new(4))],
    )
}

fn ident(kind: KindId) -> NormalizedNode {
    NormalizedNode::new(kind, Some(LeafClass::Ident), Vec::new())
}

fn literal(kind: KindId) -> NormalizedNode {
    NormalizedNode::new(kind, Some(LeafClass::Literal), Vec::new())
}

#[rstest]
fn feature_functions_reflect_tree_contents() -> AstResult<()> {
    let expected = feature_tree()?;
    let distinct = tree_with_root(NormalizedNode::new(
        KindId::new(9),
        None,
        vec![literal(KindId::new(8))],
    ))?;

    assert_ne!(kind_counts(&expected), kind_counts(&distinct));
    assert_ne!(kind_histogram(&expected), kind_histogram(&distinct));
    assert_ne!(
        production_multiset(&expected),
        production_multiset(&distinct)
    );
    assert_ne!(canonical_hash(&expected), canonical_hash(&distinct));
    Ok(())
}

proptest! {

    #[test]
    fn count_and_production_features_ignore_sibling_visit_order(
        root in normalized_node_strategy()
    ) {
        let tree = tree_with_root(root.clone()).expect("static test span should be valid");
        let reversed = tree_with_root(reverse_siblings(&root))
            .expect("static test span should be valid");

        prop_assert_eq!(kind_counts(&tree), kind_counts(&reversed));
        prop_assert_eq!(
            weighted_histogram(&kind_counts(&tree)),
            weighted_histogram(&kind_counts(&reversed))
        );
        prop_assert_eq!(production_multiset(&tree), production_multiset(&reversed));
    }

    #[test]
    fn different_kind_ids_with_same_leaf_have_different_hashes(
        kind in 0_u16..u16::MAX,
        leaf in leaf_class_strategy()
    ) {
        let left = tree_with_root(NormalizedNode::new(KindId::new(kind), leaf, Vec::new()))
            .expect("static test span should be valid");
        let right = tree_with_root(NormalizedNode::new(KindId::new(kind + 1), leaf, Vec::new()))
            .expect("static test span should be valid");

        prop_assert_ne!(canonical_hash(&left), canonical_hash(&right));
    }
}

fn normalized_node_strategy() -> impl Strategy<Value = NormalizedNode> {
    (0_u16..32, leaf_class_strategy())
        .prop_map(|(kind, leaf)| NormalizedNode::new(KindId::new(kind), leaf, Vec::new()))
        .prop_recursive(3, 32, 3, |inner| {
            (0_u16..32, prop::collection::vec(inner, 0..3))
                .prop_map(|(kind, children)| NormalizedNode::new(KindId::new(kind), None, children))
        })
}

fn leaf_class_strategy() -> impl Strategy<Value = Option<LeafClass>> {
    prop_oneof![
        Just(None),
        Just(Some(LeafClass::Ident)),
        Just(Some(LeafClass::Literal)),
        Just(Some(LeafClass::Other)),
    ]
}

fn reverse_siblings(node: &NormalizedNode) -> NormalizedNode {
    let mut children = node
        .children()
        .iter()
        .map(reverse_siblings)
        .collect::<Vec<_>>();
    children.reverse();
    NormalizedNode::new(node.kind(), node.leaf(), children)
}
