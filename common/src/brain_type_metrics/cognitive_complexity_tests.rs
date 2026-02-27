//! Unit tests for [`super::CognitiveComplexityBuilder`].

use super::*;
use rstest::rstest;

// ---------------------------------------------------------------------------
// Individual increment types — non-expansion
// ---------------------------------------------------------------------------

#[rstest]
fn structural_increment_adds_one() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false);
    assert_eq!(cc.build(), 1);
}

#[rstest]
fn nesting_increment_at_depth_zero_adds_zero() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_nesting_increment(false);
    assert_eq!(cc.build(), 0);
}

#[rstest]
fn nesting_increment_at_depth_one_adds_one() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(false);
    cc.record_nesting_increment(false);
    cc.pop_nesting();
    assert_eq!(cc.build(), 1);
}

#[rstest]
fn nesting_increment_at_depth_two_adds_two() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(false);
    cc.push_nesting(false);
    cc.record_nesting_increment(false);
    cc.pop_nesting();
    cc.pop_nesting();
    assert_eq!(cc.build(), 2);
}

#[rstest]
fn fundamental_increment_adds_one() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_fundamental_increment(false);
    assert_eq!(cc.build(), 1);
}

// ---------------------------------------------------------------------------
// Macro-expansion filtering
// ---------------------------------------------------------------------------

#[rstest]
fn structural_from_expansion_skipped() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(true);
    assert_eq!(cc.build(), 0);
}

#[rstest]
fn nesting_from_expansion_skipped() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(false);
    cc.record_nesting_increment(true);
    cc.pop_nesting();
    assert_eq!(cc.build(), 0);
}

#[rstest]
fn fundamental_from_expansion_skipped() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_fundamental_increment(true);
    assert_eq!(cc.build(), 0);
}

#[rstest]
fn mixed_real_and_expansion_increments() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false); // +1
    cc.record_structural_increment(true); // skipped
    cc.record_structural_increment(false); // +1
    assert_eq!(cc.build(), 2);
}

// ---------------------------------------------------------------------------
// Nesting stack behaviour
// ---------------------------------------------------------------------------

#[rstest]
fn push_false_increases_effective_depth() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(false);
    assert_eq!(cc.effective_depth(), 1);
    cc.pop_nesting();
}

#[rstest]
fn push_true_does_not_increase_depth() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(true);
    assert_eq!(cc.effective_depth(), 0);
    cc.pop_nesting();
}

#[rstest]
fn pop_after_push_false_decreases_depth() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(false);
    cc.pop_nesting();
    assert_eq!(cc.effective_depth(), 0);
}

#[rstest]
fn pop_after_push_true_leaves_depth() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(true);
    cc.pop_nesting();
    assert_eq!(cc.effective_depth(), 0);
}

#[rstest]
fn nested_push_pop_returns_to_zero() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(false);
    cc.push_nesting(false);
    assert_eq!(cc.effective_depth(), 2);
    cc.pop_nesting();
    assert_eq!(cc.effective_depth(), 1);
    cc.pop_nesting();
    assert_eq!(cc.effective_depth(), 0);
}

#[rstest]
fn mixed_expansion_nesting() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(false); // depth = 1
    cc.push_nesting(true); // depth still 1 (macro level)
    cc.push_nesting(false); // depth = 2
    assert_eq!(cc.effective_depth(), 2);
    cc.pop_nesting();
    cc.pop_nesting();
    cc.pop_nesting();
}

// ---------------------------------------------------------------------------
// Composite scenarios modelling real code patterns
// ---------------------------------------------------------------------------

/// Parameterised composite scenarios. Each case simulates a code
/// pattern and asserts the expected CC score.
///
/// Tuple: `(label, setup_fn, expected_score)`
#[rstest]
#[case("simple_if", 1)]
fn composite_simple_if(#[case] _label: &str, #[case] expected: usize) {
    // if cond {}  => structural +1, nesting +0 (depth 0)
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false);
    cc.record_nesting_increment(false);
    cc.push_nesting(false);
    cc.pop_nesting();
    assert_eq!(cc.build(), expected);
}

#[rstest]
#[case("nested_if_in_if", 3)]
fn composite_nested_if(#[case] _label: &str, #[case] expected: usize) {
    // if { if {} }
    // outer: struct +1, nest +0 (depth 0)
    // inner: struct +1, nest +1 (depth 1)
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false); // +1
    cc.record_nesting_increment(false); // +0
    cc.push_nesting(false);
    cc.record_structural_increment(false); // +1
    cc.record_nesting_increment(false); // +1
    cc.push_nesting(false);
    cc.pop_nesting();
    cc.pop_nesting();
    assert_eq!(cc.build(), expected);
}

#[rstest]
#[case("if_with_boolean_ops", 3)]
fn composite_if_with_boolean_ops(#[case] _label: &str, #[case] expected: usize) {
    // if a && b || c {}
    // structural +1, fundamental +1 (&&), fundamental +1 (||)
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false); // +1
    cc.record_fundamental_increment(false); // +1
    cc.record_fundamental_increment(false); // +1
    cc.push_nesting(false);
    cc.pop_nesting();
    assert_eq!(cc.build(), expected);
}

#[rstest]
#[case("triple_nested_loop", 6)]
fn composite_triple_nested_loop(#[case] _label: &str, #[case] expected: usize) {
    // for { for { for {} } }
    // L1: struct +1, nest +0 (depth 0), push => depth 1
    // L2: struct +1, nest +1 (depth 1), push => depth 2
    // L3: struct +1, nest +2 (depth 2), push => depth 3
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false); // +1
    cc.record_nesting_increment(false); // +0
    cc.push_nesting(false);
    cc.record_structural_increment(false); // +1
    cc.record_nesting_increment(false); // +1
    cc.push_nesting(false);
    cc.record_structural_increment(false); // +1
    cc.record_nesting_increment(false); // +2
    cc.push_nesting(false);
    cc.pop_nesting();
    cc.pop_nesting();
    cc.pop_nesting();
    assert_eq!(cc.build(), expected);
}

#[rstest]
#[case("macro_if_inside_real_for", 1)]
fn composite_macro_if_inside_real_for(#[case] _label: &str, #[case] expected: usize) {
    // for { MACRO_IF }
    // for: struct +1, nest +0, push(false) => depth 1
    // macro if: struct(true) skipped, nest(true) skipped
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false); // +1 (for)
    cc.record_nesting_increment(false); // +0
    cc.push_nesting(false);
    cc.record_structural_increment(true); // skipped (macro if)
    cc.record_nesting_increment(true); // skipped
    cc.push_nesting(true);
    cc.pop_nesting();
    cc.pop_nesting();
    assert_eq!(cc.build(), expected);
}

#[rstest]
#[case("real_if_inside_macro_for", 1)]
fn composite_real_if_inside_macro_for(#[case] _label: &str, #[case] expected: usize) {
    // MACRO_FOR { if {} }
    // macro for: struct(true) skipped, push(true) => eff depth 0
    // real if: struct(false) +1, nest(false) +0 (eff depth is 0)
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(true); // skipped (macro for)
    cc.record_nesting_increment(true); // skipped
    cc.push_nesting(true); // eff depth stays 0
    cc.record_structural_increment(false); // +1 (real if)
    cc.record_nesting_increment(false); // +0 (eff depth is 0)
    cc.push_nesting(false);
    cc.pop_nesting();
    cc.pop_nesting();
    assert_eq!(cc.build(), expected);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[rstest]
fn empty_builder_returns_zero() {
    assert_eq!(CognitiveComplexityBuilder::new().build(), 0);
}

#[rstest]
fn score_accessor_matches_build() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false);
    cc.record_fundamental_increment(false);
    let score = cc.score();
    assert_eq!(score, cc.build());
}

#[rstest]
fn default_matches_new() {
    let from_default = CognitiveComplexityBuilder::default().build();
    let from_new = CognitiveComplexityBuilder::new().build();
    assert_eq!(from_default, from_new);
}

#[rstest]
#[should_panic(expected = "unbalanced nesting")]
fn build_panics_on_unbalanced_stack() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.push_nesting(false);
    let _ = cc.build();
}

#[rstest]
#[should_panic(expected = "empty nesting stack")]
fn pop_panics_on_empty_stack() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.pop_nesting();
}

#[rstest]
fn multiple_structural_increments_accumulate() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(false);
    cc.record_structural_increment(false);
    cc.record_structural_increment(false);
    assert_eq!(cc.build(), 3);
}

#[rstest]
fn all_increments_from_expansion_yields_zero() {
    let mut cc = CognitiveComplexityBuilder::new();
    cc.record_structural_increment(true);
    cc.record_nesting_increment(true);
    cc.record_fundamental_increment(true);
    cc.push_nesting(true);
    cc.pop_nesting();
    assert_eq!(cc.build(), 0);
}
