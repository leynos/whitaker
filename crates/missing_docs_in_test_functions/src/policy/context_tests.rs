//! Deterministic cases for the container stack and its classification.
//!
//! The property test beside this module covers unbounded nesting; these cases
//! pin the one stack operation whose failure mode is a panic and each
//! `FunctionContext` variant's smallest witness.

use rstest::rstest;

use super::{ContainerKind, ContainerStack, FunctionContext, FunctionShape};

/// Builds a stack holding `frames` from outermost to innermost.
fn stack_of(frames: &[ContainerKind]) -> ContainerStack {
    let mut stack = ContainerStack::default();
    for kind in frames {
        stack.enter(*kind);
    }
    stack
}

/// `exit` on an empty stack returns `None` and leaves the stack empty.
#[test]
fn exit_on_an_empty_stack_returns_none() {
    let mut stack = ContainerStack::default();
    assert_eq!(
        stack.exit(),
        None,
        "popping an empty container stack must report nothing rather than panic"
    );
    assert_eq!(
        stack,
        ContainerStack::default(),
        "a failed pop must not leave a frame behind"
    );
}

/// `enter` then `exit` returns the frame that was entered.
#[test]
fn enter_then_exit_round_trips_the_frame() {
    let mut stack = ContainerStack::default();
    stack.enter(ContainerKind::TraitImpl);
    assert_eq!(
        stack.exit(),
        Some(ContainerKind::TraitImpl),
        "the popped frame must be the one just pushed"
    );
}

/// Each shape and enclosing-frame sequence classifies as the contract states.
#[rstest]
#[case(&[], FunctionShape::Free, FunctionContext::Module)]
#[case(&[ContainerKind::Module], FunctionShape::Free, FunctionContext::Module)]
#[case(
    &[ContainerKind::Module, ContainerKind::Module],
    FunctionShape::Free,
    FunctionContext::Module
)]
#[case(&[ContainerKind::Other], FunctionShape::Free, FunctionContext::Nested)]
#[case(
    &[ContainerKind::Module, ContainerKind::Other],
    FunctionShape::Free,
    FunctionContext::Nested
)]
#[case(&[ContainerKind::TraitImpl], FunctionShape::Free, FunctionContext::Nested)]
#[case(
    &[ContainerKind::InherentImpl],
    FunctionShape::Associated,
    FunctionContext::InherentImpl
)]
#[case(
    &[ContainerKind::Module, ContainerKind::InherentImpl],
    FunctionShape::Associated,
    FunctionContext::InherentImpl
)]
#[case(
    &[ContainerKind::Trait],
    FunctionShape::Associated,
    FunctionContext::Trait
)]
#[case(
    &[ContainerKind::TraitImpl],
    FunctionShape::Associated,
    FunctionContext::TraitImpl
)]
#[case(
    &[ContainerKind::Other, ContainerKind::InherentImpl],
    FunctionShape::Associated,
    FunctionContext::Nested
)]
#[case(
    &[ContainerKind::Module, ContainerKind::Other, ContainerKind::Trait],
    FunctionShape::Associated,
    FunctionContext::Nested
)]
#[case(&[], FunctionShape::Associated, FunctionContext::Nested)]
#[case(
    &[ContainerKind::Module],
    FunctionShape::Associated,
    FunctionContext::Nested
)]
fn classification_matches_the_contract(
    #[case] frames: &[ContainerKind],
    #[case] shape: FunctionShape,
    #[case] expected: FunctionContext,
) {
    let stack = stack_of(frames);
    assert_eq!(
        stack.classify(shape),
        expected,
        "frames {frames:?} and shape {shape:?} must classify as {expected:?}",
    );
}
