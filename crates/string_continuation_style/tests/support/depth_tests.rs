//! Tests that nested expansion depth is restored after each walk.

use super::WrapperDepth;
use crate::exclusion::PathExclusions;

#[rstest::rstest]
#[case::source_containers(&[false, false], &[Some(0), Some(0)])]
#[case::one_expansion(&[true, false], &[Some(1), Some(1)])]
#[case::nested_expansion(&[true, true, false], &[Some(1), Some(2), Some(2)])]
fn expansion_depth_tracks_synthetic_wrapper_sequences(
    #[case] sequence: &[bool],
    #[case] expected: &[Option<usize>],
) {
    let exclusions = PathExclusions::default();
    let mut visitor = WrapperDepth::new("sample", &exclusions);
    let mut observed = Vec::new();

    observe_sequence(&mut visitor, sequence, &mut observed);

    assert_eq!(observed, expected);
    assert_eq!(visitor.expansion_depth, Some(0));
}

fn observe_sequence(
    visitor: &mut WrapperDepth<'_>,
    sequence: &[bool],
    observed: &mut Vec<Option<usize>>,
) {
    let Some((&is_expansion, nested)) = sequence.split_first() else {
        return;
    };
    visitor.with_expansion(is_expansion, |visitor| {
        observed.push(visitor.expansion_depth);
        observe_sequence(visitor, nested, observed);
    });
}

#[test]
fn depth_overflow_fails_closed_until_the_container_exits() {
    let exclusions = PathExclusions::default();
    let mut visitor = WrapperDepth::new("sample", &exclusions);
    visitor.expansion_depth = Some(usize::MAX);
    let mut observed = None;

    visitor.with_expansion(true, |nested| observed = nested.expansion_depth);

    assert_eq!(observed, None);
    assert_eq!(visitor.expansion_depth, Some(usize::MAX));
}
