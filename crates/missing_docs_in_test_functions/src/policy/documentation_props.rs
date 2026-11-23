//! Property and table tests for the documentation and context predicates.
//!
//! These are separated from the exhaustive selection enumeration because they
//! vary one input family at a time: the attribute shapes the documentation gate
//! reads, and the container frames the context classification reads. Keeping
//! them beside that enumeration pushed it past the repository's 400-line limit.

use proptest::prelude::*;
use rstest::rstest;

use super::{
    ContainerKind,
    ContainerStack,
    DocAttributeShape,
    Documentation,
    FunctionContext,
    FunctionShape,
    documentation_status,
};

/// Documentation shapes classify as their documented contract states.
#[rstest]
#[case(DocAttributeShape::DocComment, true)]
#[case(DocAttributeShape::DocNameValue, true)]
#[case(DocAttributeShape::CfgAttrDoc, true)]
#[case(DocAttributeShape::DocList, false)]
#[case(DocAttributeShape::Other, false)]
fn documentation_shapes_classify(#[case] shape: DocAttributeShape, #[case] is_docs: bool) {
    assert_eq!(
        documentation_status([shape]),
        if is_docs {
            Documentation::Documented
        } else {
            Documentation::Undocumented
        },
        "shape {shape:?} must classify as documentation == {is_docs}",
    );
}

proptest! {
    /// For any set of attribute shapes, the function is documented exactly when
    /// at least one shape counts as documentation.
    #[test]
    fn documentation_status_is_an_existential_over_shapes(
        shapes in prop::collection::vec(any_shape(), 0..=8_usize)
    ) {
        let expected = shapes.iter().any(|shape| shape.counts_as_documentation());
        prop_assert_eq!(
            documentation_status(shapes.clone()) == Documentation::Documented,
            expected,
        );
    }

    /// A free function is in scope exactly when every frame is a module.
    #[test]
    fn free_function_context_depends_only_on_module_frames(
        kinds in prop::collection::vec(any_container_kind(), 0..=6_usize)
    ) {
        let mut stack = ContainerStack::default();
        for kind in &kinds {
            stack.enter(*kind);
        }
        let expected = if kinds.iter().all(|kind| *kind == ContainerKind::Module) {
            FunctionContext::Module
        } else {
            FunctionContext::Nested
        };
        prop_assert_eq!(stack.classify(FunctionShape::Free), expected);
    }
}

/// Generates one `DocAttributeShape` value.
fn any_shape() -> impl Strategy<Value = DocAttributeShape> {
    prop_oneof![
        Just(DocAttributeShape::DocComment),
        Just(DocAttributeShape::DocNameValue),
        Just(DocAttributeShape::CfgAttrDoc),
        Just(DocAttributeShape::DocList),
        Just(DocAttributeShape::Other),
    ]
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
