//! Bounded verification harnesses for AST feature extraction.
//!
//! These harnesses never invoke `ra_ap_syntax`; they exercise production
//! parser-independent helpers over small synthetic spans and lowered trees.

use std::ops::Range;

use super::{
    ByteSpan,
    KindId,
    LeafClass,
    NormalizedNode,
    NormalizedTree,
    features::for_each_node_at_depth,
    select_smallest_covering,
};

const KANI_AST_MAX_DEPTH: usize = 3;
const KANI_AST_MAX_CHILDREN: usize = 2;
const KANI_AST_NODE_COUNT: usize = 5;
const KANI_AST_UNWIND: usize = KANI_AST_NODE_COUNT + 1;
const _: () = assert!(KANI_AST_MAX_CHILDREN == 2);
const _: () = assert!(KANI_AST_UNWIND == 6);

/// Create a symbolic kind from four ordered ids.
fn symbolic_kind_from_small_alphabet() -> KindId {
    // Two symbolic bits produce four ordered ids, enough to realize every
    // equality partition and ordering of the root, two child, and query labels.
    // The full-width `KindId` round trip is proved in a separate harness.
    let low_bit: bool = kani::any();
    let high_bit: bool = kani::any();
    let raw = u16::from(low_bit) | (u16::from(high_bit) << 1);
    KindId::new(raw)
}

/// Assert that `KindId` comparisons agree with comparisons of their raw ids.
fn assert_kind_comparisons_match_raw(left: KindId, right: KindId) {
    kani::assert(
        (left == right) == (left.get() == right.get()),
        "KindId equality must preserve raw-id equality",
    );
    kani::assert(
        left.cmp(&right) == left.get().cmp(&right.get()),
        "KindId ordering must preserve raw-id ordering",
    );
}

/// Build the valid static span used by synthetic AST fixtures.
fn ast_span() -> ByteSpan {
    match ByteSpan::new("abcd", 0, 1) {
        Ok(span) => span,
        Err(error) => panic!("fixed Kani AST span must validate, got {error:?}"),
    }
}

/// Build a leaf node with the requested kind and class.
fn leaf(kind: KindId, leaf: LeafClass) -> NormalizedNode {
    NormalizedNode::new(kind, Some(leaf), Vec::new())
}

/// Build a branch node with the requested kind and children.
fn branch(kind: KindId, children: Vec<NormalizedNode>) -> NormalizedNode {
    NormalizedNode::new(kind, None, children)
}

struct BranchSpec {
    kind: KindId,
    leaf_kind: KindId,
}

/// Build a five-node tree whose leaves sit at depth two.
fn bounded_tree(
    root_kind: KindId,
    left: BranchSpec,
    right: BranchSpec,
    left_first: bool,
) -> NormalizedTree {
    let left_leaf = leaf(left.leaf_kind, LeafClass::Ident);
    let right_leaf = leaf(right.leaf_kind, LeafClass::Literal);
    let left_branch = branch(left.kind, vec![left_leaf]);
    let right_branch = branch(right.kind, vec![right_leaf]);
    let children = if left_first {
        vec![left_branch, right_branch]
    } else {
        vec![right_branch, left_branch]
    };

    NormalizedTree::new(branch(root_kind, children), ast_span())
}

/// Generate a valid range within the bounded offset domain.
fn symbolic_range() -> Range<u32> {
    let start: u32 = kani::any();
    let end: u32 = kani::any();
    kani::assume(start <= 16);
    kani::assume(end <= 16);
    kani::assume(start < end);
    start..end
}

/// Return whether `candidate` encloses `target`.
fn covers(candidate: &Range<u32>, target: &Range<u32>) -> bool {
    candidate.start <= target.start && candidate.end >= target.end
}

/// Compare the lengths of two valid ranges.
fn strictly_smaller(left: &Range<u32>, right: &Range<u32>) -> bool {
    left.end - left.start < right.end - right.start
}

/// Assert that `selected` is a smallest candidate covering `target`.
fn assert_minimal_cover(candidates: &[Range<u32>], target: &Range<u32>, selected: usize) {
    let selected_candidate = &candidates[selected];
    kani::assert(
        covers(selected_candidate, target),
        "selected candidate must cover the target",
    );

    for candidate in candidates {
        if covers(candidate, target) {
            kani::assert(
                !strictly_smaller(candidate, selected_candidate),
                "no covering candidate may be strictly smaller than the selected candidate",
            );
        }
    }
}

/// Verify selection returns a smallest covering range.
#[kani::proof]
#[kani::unwind(5)]
fn verify_smallest_covering_node_selects_minimal_range() {
    let candidates = [symbolic_range(), symbolic_range(), symbolic_range()];
    let target = symbolic_range();
    let n = candidates.len();
    kani::assume(n >= 2);
    kani::assume(
        candidates
            .iter()
            .any(|candidate| covers(candidate, &target)),
    );

    match select_smallest_covering(&candidates, &target) {
        Some(selected) => assert_minimal_cover(&candidates, &target, selected),
        None => kani::assert(false, "covering candidate must be selected"),
    }
}

/// Verify missing covering candidates leave selection empty.
#[kani::proof]
#[kani::unwind(5)]
fn verify_missing_cover_returns_none() {
    let empty: [Range<u32>; 0] = [];
    let target = 4..8;
    kani::assert(
        select_smallest_covering(&empty, &target).is_none(),
        "empty candidate sets must leave selection empty",
    );

    let candidates = [0..2, 9..12, 12..16];
    kani::assert(
        candidates
            .iter()
            .all(|candidate| !covers(candidate, &target)),
        "fallback fixture must contain no covering candidates",
    );
    kani::assert(
        select_smallest_covering(&candidates, &target).is_none(),
        "non-covering candidate sets must leave selection empty",
    );
}

/// Verify `KindId` preserves every bit of its raw `u16` id.
#[kani::proof]
#[kani::unwind(1)]
fn verify_kind_id_round_trips_full_u16_domain() {
    let raw: u16 = kani::any();
    let kind = KindId::new(raw);
    kani::assert(kind.get() == raw, "KindId must preserve the lowered index");
}

/// Verify traversal does not observe nodes beyond the configured depth.
///
/// Six unwind steps cover five node visits and the final termination check.
/// The child loop needs at most three steps (two children plus exhaustion),
/// including its iterator helpers. Fixed kinds keep this proof focused on
/// depth; the production `BTreeMap` is exercised by runtime and property tests.
/// Unwinding assertions remain enabled.
#[kani::proof]
#[kani::unwind(6)]
fn verify_kind_counts_stop_at_max_depth() {
    let kind = KindId::new(1);
    let tree = bounded_tree(
        kind,
        BranchSpec {
            kind,
            leaf_kind: kind,
        },
        BranchSpec {
            kind,
            leaf_kind: kind,
        },
        true,
    );
    let mut depth_counts = [0_u8; KANI_AST_MAX_DEPTH + 1];
    let mut observed_beyond_max_depth = false;
    for_each_node_at_depth(tree.root(), |_, depth| {
        let depth_index = usize::from(depth.get());
        if depth_index < depth_counts.len() {
            depth_counts[depth_index] = depth_counts[depth_index].saturating_add(1);
        } else {
            observed_beyond_max_depth = true;
        }
    });
    kani::assert(
        !observed_beyond_max_depth,
        "bounded trees must contain no nodes beyond KANI_AST_MAX_DEPTH",
    );
    kani::assert(
        depth_counts[0] == 1
            && depth_counts[1] == 2
            && depth_counts[2] == 2
            && depth_counts[3] == 0,
        "the depth-only fixture must count all five nodes",
    );
}

/// Verify `KindId` equality and ordering over the documented small alphabet.
#[kani::proof]
#[kani::unwind(1)]
fn verify_kind_id_equality_and_order_cover_small_alphabet() {
    let root_kind = symbolic_kind_from_small_alphabet();
    let left_kind = symbolic_kind_from_small_alphabet();
    let right_kind = symbolic_kind_from_small_alphabet();
    let query_kind = symbolic_kind_from_small_alphabet();

    assert_kind_comparisons_match_raw(root_kind, left_kind);
    assert_kind_comparisons_match_raw(root_kind, right_kind);
    assert_kind_comparisons_match_raw(root_kind, query_kind);
    assert_kind_comparisons_match_raw(left_kind, right_kind);
    assert_kind_comparisons_match_raw(left_kind, query_kind);
    assert_kind_comparisons_match_raw(right_kind, query_kind);
}
