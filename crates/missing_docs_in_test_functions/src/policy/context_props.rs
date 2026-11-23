//! Property test tying the container stack to an independent tree oracle.
//!
//! Nesting depth is unbounded, so enumeration is impossible. The oracle here
//! recurses over a generated item tree and classifies each function from its
//! ancestor path; the implementation under test is [`ContainerStack`], driven
//! by the event sequence rustc's visitor produces for the same tree. The two
//! differ in shape — path recursion versus a running stack — so a driver that
//! pushes the wrong frame, forgets a push, or classifies by the innermost frame
//! alone disagrees with the oracle.

use std::{cell::RefCell, collections::HashSet};

use proptest::{prelude::*, test_runner::TestRunner};

use super::{ContainerKind, ContainerStack, FunctionContext, FunctionShape};

/// One node of the generated item tree.
#[derive(Clone, Debug)]
enum ItemNode {
    /// An item that contains other items.
    Container {
        /// The container's kind.
        kind: ContainerKind,
        /// The items written inside it.
        children: Vec<Self>,
    },
    /// A function leaf.
    Function(FunctionShape),
}

/// One event in the visitor order rustc produces for a tree.
#[derive(Clone, Copy, Debug)]
enum Event {
    /// A container's children are about to be walked.
    Enter(ContainerKind),
    /// A container's children have been walked.
    Exit,
    /// A function's hook has been reached.
    Visit(FunctionShape),
}

/// Generates an arbitrary item tree bounded to depth 6 and breadth 4.
fn any_item_node() -> impl Strategy<Value = ItemNode> {
    let leaf = prop_oneof![
        Just(ItemNode::Function(FunctionShape::Free)),
        Just(ItemNode::Function(FunctionShape::Associated)),
    ];
    leaf.prop_recursive(6, 64, 4, |inner| {
        (
            any_container_kind(),
            prop::collection::vec(inner, 0..=4_usize),
        )
            .prop_map(|(kind, children)| ItemNode::Container { kind, children })
    })
}

/// Generates one `ContainerKind` value.
fn any_container_kind() -> impl Strategy<Value = ContainerKind> {
    prop_oneof![
        Just(ContainerKind::Module),
        Just(ContainerKind::InherentImpl),
        Just(ContainerKind::TraitImpl),
        Just(ContainerKind::Trait),
        Just(ContainerKind::Other),
    ]
}

/// Linearizes a tree into the events rustc's visitor would produce.
fn linearize(node: &ItemNode, events: &mut Vec<Event>) {
    match node {
        ItemNode::Container { kind, children } => {
            events.push(Event::Enter(*kind));
            for child in children {
                linearize(child, events);
            }
            events.push(Event::Exit);
        }
        ItemNode::Function(shape) => events.push(Event::Visit(*shape)),
    }
}

/// Classifies each function in the tree from its ancestor path.
fn oracle_contexts(
    node: &ItemNode,
    ancestors: &mut Vec<ContainerKind>,
    out: &mut Vec<FunctionContext>,
) {
    match node {
        ItemNode::Container { kind, children } => {
            ancestors.push(*kind);
            for child in children {
                oracle_contexts(child, ancestors, out);
            }
            ancestors.pop();
        }
        ItemNode::Function(shape) => out.push(oracle_classify(ancestors, *shape)),
    }
}

/// The independent classification rule: a function is in scope only when every
/// ancestor before its parent is a module.
fn oracle_classify(ancestors: &[ContainerKind], shape: FunctionShape) -> FunctionContext {
    let every_leading_ancestor_is_a_module =
        |leading: &[ContainerKind]| leading.iter().all(|kind| *kind == ContainerKind::Module);
    match shape {
        FunctionShape::Free => {
            if every_leading_ancestor_is_a_module(ancestors) {
                FunctionContext::Module
            } else {
                FunctionContext::Nested
            }
        }
        FunctionShape::Associated => {
            let Some((parent, leading)) = ancestors.split_last() else {
                return FunctionContext::Nested;
            };
            if !every_leading_ancestor_is_a_module(leading) {
                return FunctionContext::Nested;
            }
            match parent {
                ContainerKind::InherentImpl => FunctionContext::InherentImpl,
                ContainerKind::Trait => FunctionContext::Trait,
                ContainerKind::TraitImpl => FunctionContext::TraitImpl,
                ContainerKind::Module | ContainerKind::Other => FunctionContext::Nested,
            }
        }
    }
}

/// Classifies each visited function by running the production stack over the
/// event sequence.
fn stack_contexts(events: &[Event]) -> Vec<FunctionContext> {
    let mut stack = ContainerStack::default();
    let mut out = Vec::new();
    for event in events {
        match event {
            Event::Enter(kind) => stack.enter(*kind),
            Event::Exit => drop(stack.exit()),
            Event::Visit(shape) => out.push(stack.classify(*shape)),
        }
    }
    out
}

proptest! {
    /// `L-STACK`: the stack's classification equals the tree oracle's for every
    /// function in every generated tree, and the stack ends empty.
    #[test]
    fn stack_classification_matches_the_tree_oracle(node in any_item_node()) {
        let mut events = Vec::new();
        linearize(&node, &mut events);

        let mut expected = Vec::new();
        oracle_contexts(&node, &mut Vec::new(), &mut expected);

        let actual = stack_contexts(&events);
        prop_assert_eq!(&actual, &expected);

        let mut stack = ContainerStack::default();
        for event in &events {
            match event {
                Event::Enter(kind) => stack.enter(*kind),
                Event::Exit => drop(stack.exit()),
                Event::Visit(_) => {}
            }
        }
        prop_assert_eq!(stack, ContainerStack::default());
    }
}

/// The generated domain reaches every classification, so the property is not
/// vacuously true over a narrow slice of trees.
///
/// A fixed number of trees is drawn from the same strategy with a
/// fixed-seed runner; the assertion is on coverage, not on the property.
#[test]
fn generated_trees_reach_every_classification() {
    let mut runner = TestRunner::default();
    let observed: RefCell<HashSet<FunctionContext>> = RefCell::new(HashSet::new());
    let saw_other_container = RefCell::new(false);
    let reached_depth = RefCell::new(0_usize);
    let draw = any_item_node();

    runner
        .run(&draw, |node| {
            let mut events = Vec::new();
            linearize(&node, &mut events);
            for context in stack_contexts(&events) {
                observed.borrow_mut().insert(context);
            }
            if contains_other(&node) {
                *saw_other_container.borrow_mut() = true;
            }
            let depth = depth_of(&node);
            let deepest_so_far = *reached_depth.borrow();
            *reached_depth.borrow_mut() = deepest_so_far.max(depth);
            Ok(())
        })
        .expect("the fixed-seed draw must not fail");

    let observed_contexts = observed.into_inner();
    for expected in [
        FunctionContext::Module,
        FunctionContext::InherentImpl,
        FunctionContext::Trait,
        FunctionContext::TraitImpl,
        FunctionContext::Nested,
    ] {
        assert!(
            observed_contexts.contains(&expected),
            "the generator must reach {expected:?}; observed {observed_contexts:?}",
        );
    }
    assert!(
        saw_other_container.into_inner(),
        "the generator must produce an `Other` container"
    );
    assert!(
        reached_depth.into_inner() >= 3,
        "the generator must reach nesting depth 3 or more"
    );
}

/// Reports whether the tree holds an `Other` container.
fn contains_other(node: &ItemNode) -> bool {
    match node {
        ItemNode::Container { kind, children } => {
            *kind == ContainerKind::Other || children.iter().any(contains_other)
        }
        ItemNode::Function(_) => false,
    }
}

/// Returns the tree's maximum nesting depth.
fn depth_of(node: &ItemNode) -> usize {
    match node {
        ItemNode::Container { children, .. } => {
            1 + children.iter().map(depth_of).max().unwrap_or(0)
        }
        ItemNode::Function(_) => 0,
    }
}
