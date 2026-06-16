//! Bounded verification harnesses for AST feature extraction.
//!
//! These harnesses never invoke `ra_ap_syntax`; they exercise production
//! parser-independent helpers over small synthetic spans and lowered trees.

use std::ops::Range;

use super::{
    ByteSpan, Depth, KindId, LeafClass, NormalisedNode, NormalisedTree, select_smallest_covering,
};

const KANI_AST_MAX_DEPTH: usize = 3;
const KANI_AST_MAX_CHILDREN: usize = 2;
const KANI_AST_UNWIND: usize = 5;
const _: () = assert!(KANI_AST_MAX_CHILDREN == 2);
const _: () = assert!(KANI_AST_UNWIND == KANI_AST_MAX_DEPTH + 2);

fn symbolic_kind() -> KindId {
    KindId::new(kani::any())
}

fn ast_span() -> ByteSpan {
    match ByteSpan::new("abcd", 0, 1) {
        Ok(span) => span,
        Err(error) => panic!("fixed Kani AST span must validate, got {error:?}"),
    }
}

fn leaf(kind: KindId, leaf: LeafClass) -> NormalisedNode {
    NormalisedNode::new(kind, Some(leaf), Vec::new())
}

fn branch(kind: KindId, children: Vec<NormalisedNode>) -> NormalisedNode {
    NormalisedNode::new(kind, None, children)
}

fn bounded_tree(
    root_kind: KindId,
    left_branch_kind: KindId,
    right_branch_kind: KindId,
    left_leaf_kind: KindId,
    right_leaf_kind: KindId,
    left_first: bool,
) -> NormalisedTree {
    let left_leaf = leaf(left_leaf_kind, LeafClass::Ident);
    let right_leaf = leaf(right_leaf_kind, LeafClass::Literal);
    let left_branch = branch(left_branch_kind, vec![left_leaf]);
    let right_branch = branch(right_branch_kind, vec![right_leaf]);
    let children = if left_first {
        vec![left_branch, right_branch]
    } else {
        vec![right_branch, left_branch]
    };

    NormalisedTree::new(branch(root_kind, children), ast_span())
}

fn symbolic_range() -> Range<u32> {
    let start: u32 = kani::any();
    let end: u32 = kani::any();
    kani::assume(start <= 16);
    kani::assume(end <= 16);
    kani::assume(start < end);
    start..end
}

fn covers(candidate: &Range<u32>, target: &Range<u32>) -> bool {
    candidate.start <= target.start && candidate.end >= target.end
}

fn strictly_smaller(left: &Range<u32>, right: &Range<u32>) -> bool {
    left.end - left.start < right.end - right.start
}

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

fn count_kind_at_depth(node: &NormalisedNode, kind: KindId, depth: Depth, target: Depth) -> u32 {
    let mut count: u32 = if node.kind() == kind && depth == target {
        1
    } else {
        0
    };

    for child in node.children() {
        count = count.saturating_add(count_kind_at_depth(
            child,
            kind,
            Depth::new(depth.get().saturating_add(1)),
            target,
        ));
    }

    count
}

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

#[kani::proof]
#[kani::unwind(5)]
fn verify_smallest_covering_root_fallback() {
    let empty: [Range<u32>; 0] = [];
    let target = 4..8;
    kani::assert(
        select_smallest_covering(&empty, &target).is_none(),
        "empty candidate sets must fall back to the parser root",
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
        "non-covering candidate sets must fall back to the parser root",
    );
}

#[kani::proof]
#[kani::unwind(5)]
fn verify_kind_index_is_bounded() {
    let raw: u16 = kani::any();
    let kind = KindId::new(raw);

    kani::assert(kind.get() == raw, "KindId must preserve the lowered index");
    kani::assert(
        u32::from(kind.get()) <= u32::from(u16::MAX),
        "KindId indexes must stay inside the u16 parser-kind range",
    );
    kani::assert(
        Depth::new(KANI_AST_MAX_DEPTH as u16).get() <= KANI_AST_MAX_DEPTH as u16,
        "bounded AST depth witness must stay inside the configured harness depth",
    );
}

#[kani::proof]
#[kani::unwind(5)]
fn verify_count_accumulation_is_order_independent_bounded() {
    let root_kind = symbolic_kind();
    let left_branch_kind = symbolic_kind();
    let right_branch_kind = symbolic_kind();
    let left_leaf_kind = symbolic_kind();
    let right_leaf_kind = symbolic_kind();
    let forward = bounded_tree(
        root_kind,
        left_branch_kind,
        right_branch_kind,
        left_leaf_kind,
        right_leaf_kind,
        true,
    );
    let reverse = bounded_tree(
        root_kind,
        left_branch_kind,
        right_branch_kind,
        left_leaf_kind,
        right_leaf_kind,
        false,
    );
    let query_kind = symbolic_kind();
    let query_depth = Depth::new(kani::any());
    kani::assume(query_depth.get() <= KANI_AST_MAX_DEPTH as u16);

    kani::assert(
        count_kind_at_depth(forward.root(), query_kind, Depth::root(), query_depth)
            == count_kind_at_depth(reverse.root(), query_kind, Depth::root(), query_depth),
        "kind-count contribution folds must ignore sibling visit order for the bounded tree",
    );
}
