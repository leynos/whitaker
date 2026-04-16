//! Behaviour-driven tests for shared `rstest` span recovery helpers.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use whitaker_common::rstest::{SpanRecoveryFrame, UserEditableSpan, recover_user_editable_span};
use whitaker_common::span::{SourceLocation, SourceSpan};

#[derive(Default)]
struct SpanRecoveryWorld {
    frames: RefCell<Vec<SpanRecoveryFrame<SourceSpan>>>,
    result: RefCell<Option<UserEditableSpan<SourceSpan>>>,
}

impl SpanRecoveryWorld {
    fn push_frame(&self, line: usize, from_expansion: bool) {
        let span = source_span(line);
        self.frames
            .borrow_mut()
            .push(SpanRecoveryFrame::new(span, from_expansion));
    }

    fn evaluate(&self) {
        let frames = self.frames.borrow();
        self.result
            .replace(Some(recover_user_editable_span(frames.as_slice())));
    }
}

#[fixture]
fn world() -> SpanRecoveryWorld {
    SpanRecoveryWorld::default()
}

#[given("a direct user-editable span at line {line}")]
fn given_direct_span(world: &SpanRecoveryWorld, line: usize) {
    world.push_frame(line, false);
}

#[given("a macro frame at line {line}")]
fn given_macro_frame(world: &SpanRecoveryWorld, line: usize) {
    world.push_frame(line, true);
}

#[given("a user-editable frame at line {line}")]
fn given_user_frame(world: &SpanRecoveryWorld, line: usize) {
    world.push_frame(line, false);
}

#[when("I recover the user-editable span")]
fn when_recover(world: &SpanRecoveryWorld) {
    world.evaluate();
}

fn source_span(line: usize) -> SourceSpan {
    match SourceSpan::new(SourceLocation::new(line, 1), SourceLocation::new(line, 8)) {
        Ok(span) => span,
        Err(error) => panic!("behaviour test span invalid for line {line}: {error:?}"),
    }
}

#[then("the recovery result keeps the direct span at line {line}")]
fn then_direct(world: &SpanRecoveryWorld, line: usize) {
    let expected = source_span(line);

    assert_eq!(
        *world.result.borrow(),
        Some(UserEditableSpan::Direct(expected))
    );
}

#[then("the recovery result uses a recovered span at line {line}")]
fn then_recovered(world: &SpanRecoveryWorld, line: usize) {
    let expected = source_span(line);

    assert_eq!(
        *world.result.borrow(),
        Some(UserEditableSpan::Recovered(expected))
    );
}

#[then("the recovery result is macro-only")]
fn then_macro_only(world: &SpanRecoveryWorld) {
    assert_eq!(*world.result.borrow(), Some(UserEditableSpan::MacroOnly));
}

#[scenario(path = "tests/features/rstest_span_recovery.feature", index = 0)]
fn scenario_direct_span_is_kept(world: SpanRecoveryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/rstest_span_recovery.feature", index = 1)]
fn scenario_nested_macro_chain_recovers(world: SpanRecoveryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/rstest_span_recovery.feature", index = 2)]
fn scenario_macro_only_is_skipped(world: SpanRecoveryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/rstest_span_recovery.feature", index = 3)]
fn scenario_first_user_frame_wins(world: SpanRecoveryWorld) {
    let _ = world;
}
