//! Refinement tests for classification rows and determinism.
//!
//! Split from the sibling test module to stay inside the 400-line module
//! budget. The shared fixtures live in the sibling and are imported here.

use std::collections::BTreeMap;

use rstest::rstest;

use super::{
    refine_with_lowering,
    tests::{NODES, ast_config, fragment, pair, refine, token_config, type2_pair},
};
use crate::{
    CloneRule,
    NormProfile,
    RefinedPair,
    RefinementInput,
    Run0Error,
    Run1Error,
    test_support::{fixture_span, synthetic_tree},
};

/// Check that mixed-profile candidates are rejected.
#[test]
fn mixed_profile_candidates_are_rejected() {
    let fragments = vec![
        fragment("left", NormProfile::T1, 0..10, &[1]),
        fragment("right", NormProfile::T2, 10..20, &[1]),
    ];
    let candidates = vec![pair("left", "right")];
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &[], &token_config);

    let result = refine_with_lowering(&input, &ast_config(), |_text, span| {
        Ok(synthetic_tree(7, NODES, span))
    });

    assert!(
        matches!(
            result,
            Err(Run1Error::TokenPass(Run0Error::MixedProfiles { .. }))
        ),
        "mixed-profile candidates must be rejected, got {result:?}"
    );
}

/// Check the node floor exactly: one node fewer is inconclusive, equal is not.
///
/// The floor is `>=`, so a tree of exactly `min_nodes - 1` nodes must make the
/// evidence inconclusive while a tree of exactly `min_nodes` must not. Asserting
/// only "some small tree is inconclusive" would pass even if the comparison were
/// wrong by one.
#[test]
fn min_nodes_guard_has_an_exact_boundary() {
    let floor = ast_config().min_nodes();
    let below = synthetic_tree(7, floor.saturating_sub(1), fixture_span(0, 10));
    let at_floor = synthetic_tree(7, floor, fixture_span(0, 10));

    assert_eq!(
        crate::pair_result::node_count(&crate::ast::kind_counts(&below)),
        floor.saturating_sub(1),
        "the fixture must produce exactly the requested node count"
    );
    assert!(
        is_inconclusive(&below),
        "a tree below the floor is inconclusive"
    );
    assert!(
        !is_inconclusive(&at_floor),
        "a tree at the floor is conclusive"
    );
}

/// Check that the fixture really produces the node count it is asked for.
///
/// Every other test in this module treats `synthetic_tree`'s size as
/// authoritative, so an off-by-one in the fixture would silently move the
/// `min_nodes` boundary those tests assert. The requested sizes span the floor
/// and one either side of it, which is where an off-by-one shows up.
#[rstest]
#[case(5)]
#[case(6)]
#[case(9)]
#[case(10)]
#[case(11)]
#[case(20)]
#[case(21)]
fn synthetic_tree_produces_the_requested_node_count(#[case] nodes: usize) {
    let tree = synthetic_tree(7, nodes, fixture_span(0, 10));

    assert_eq!(
        crate::pair_result::node_count(&crate::ast::kind_counts(&tree)),
        nodes,
        "a request for {nodes} nodes must produce exactly {nodes}"
    );
}

/// Check the covering-span guard at and past its exact limit.
///
/// A lowered tree whose covering node is much larger than the requested
/// fragment means the parser recovered by swallowing surrounding code, so the
/// AST evidence describes a bigger region than the one under test. The guard
/// is `<=` against `fragment_len * max_cover_expansion`, so the case exactly at
/// the limit must stay conclusive while one byte more must not.
///
/// Without the guard the oversized tree would produce conclusive evidence here
/// — the two trees differ and score above the threshold — so the test fails if
/// the condition is removed or inverted.
#[test]
fn covering_span_guard_has_an_exact_boundary() {
    // Each fragment spans ten bytes, and the default expansion is two, so the
    // limit is twenty bytes of covering span.
    let at_limit = refine_with_trees(
        synthetic_tree(7, NODES, fixture_span(0, 20)),
        synthetic_tree(9, NODES + 1, fixture_span(10, 30)),
    );
    let over_limit = refine_with_trees(
        synthetic_tree(7, NODES, fixture_span(0, 21)),
        synthetic_tree(9, NODES + 1, fixture_span(10, 30)),
    );

    let [at_pair] = at_limit.pairs() else {
        panic!("the fixture refines one pair, got {at_limit:?}");
    };
    let RefinedPair::Carried { ast, .. } = at_pair else {
        panic!("a low-cosine pair is carried, got {at_pair:?}");
    };
    assert!(
        ast.is_some(),
        "a covering span exactly at the limit must stay conclusive"
    );

    let [over_pair] = over_limit.pairs() else {
        panic!("the fixture refines one pair, got {over_limit:?}");
    };
    let RefinedPair::Carried {
        ast: over_ast,
        token,
    } = over_pair
    else {
        panic!("an inconclusive pair is carried, got {over_pair:?}");
    };
    assert!(
        over_ast.is_none(),
        "a covering span past the limit must yield no AST evidence"
    );
    assert_eq!(
        token.rule(),
        CloneRule::Type2,
        "the token verdict must stand when the AST evidence is inconclusive"
    );
}

/// Refine the standard fixture with one tree per fragment span.
pub(super) fn refine_with_trees(
    left: crate::NormalizedTree,
    right: crate::NormalizedTree,
) -> super::Refinement {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let mut trees = BTreeMap::new();
    trees.insert(0, left);
    trees.insert(10, right);
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);
    refine(&input, &trees)
}

/// Report whether a single tree makes its pair's evidence inconclusive.
pub(super) fn is_inconclusive(tree: &crate::NormalizedTree) -> bool {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let mut trees = BTreeMap::new();
    trees.insert(0, tree.clone());
    trees.insert(10, tree.clone());
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);

    let refined = refine(&input, &trees);
    let [RefinedPair::Carried { ast, .. }] = refined.pairs() else {
        panic!("an accepted pair must be carried, got {refined:?}");
    };
    ast.is_none()
}
