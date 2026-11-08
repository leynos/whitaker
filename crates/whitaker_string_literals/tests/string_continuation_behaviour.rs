//! Behavioural scenarios for the pure string-literal domain crate.
//!
//! RFC 0002 §Behaviour scenarios states three rules in prose. Binding them to
//! `classify` here keeps the prose and the policy in step: a scenario is an
//! executable claim about the public classifier, not a restatement of its
//! internals.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_string_literals::{
    classification::{ContinuationDisposition, RequiredReason, classify},
    facts::{CaptureStatus, LiteralBody, LiteralContext, LiteralFacts},
};

/// The facts built by the `Given` steps and the outcome the `When` step records.
#[derive(Default)]
struct World {
    body: Option<&'static str>,
    context: Option<LiteralContext>,
    outcome: Option<Outcome>,
}

/// A classified result with the rewrite dropped, so it can be compared without
/// maintaining a lifetime to the body it borrows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    PreferConcat,
    RequireContinuation(RequiredReason),
    Ignore,
}

impl From<ContinuationDisposition<'_>> for Outcome {
    fn from(disposition: ContinuationDisposition<'_>) -> Self {
        match disposition {
            ContinuationDisposition::PreferConcat(_) => Self::PreferConcat,
            ContinuationDisposition::RequireContinuation(reason) => {
                Self::RequireContinuation(reason)
            }
            ContinuationDisposition::Ignore => Self::Ignore,
        }
    }
}

impl World {
    const fn set_body(&mut self, body: &'static str) { self.body = Some(body); }

    const fn set_context(&mut self, context: LiteralContext) { self.context = Some(context); }

    /// Classifies the recorded facts, panicking if no `Given` step supplied a
    /// body.
    ///
    /// The leading-continuation scenario states no context, because its claim
    /// is about layout trimming rather than about where the literal sits. A
    /// general expression is the context that claim is made in, so that is the
    /// default.
    fn classify(&mut self) {
        let Some(body) = self.body else {
            panic!("a literal body must be given before classifying")
        };
        let context = self.context.unwrap_or(LiteralContext::Expression);
        let facts = LiteralFacts::Cooked {
            body: LiteralBody::new(body),
            context,
        };
        self.outcome = Some(Outcome::from(classify(&facts)));
    }

    fn outcome(&self) -> Outcome {
        let Some(outcome) = self.outcome else {
            panic!("the continuation must be classified before checking the disposition")
        };
        outcome
    }
}

type WorldCell = RefCell<World>;

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> WorldCell { RefCell::default() }

#[given("a cooked string expression with an interior continuation")]
fn given_join(world: &WorldCell) { world.borrow_mut().set_body("alpha \\\n    beta"); }

#[given("the literal is in a general expression context")]
fn given_expression_context(world: &WorldCell) {
    world.borrow_mut().set_context(LiteralContext::Expression);
}

#[given("a source-authored format string with an interior continuation")]
fn given_format_string(world: &WorldCell) {
    let mut state = world.borrow_mut();
    state.set_body("alpha \\\n    beta");
    state.set_context(LiteralContext::SourceFormatString {
        capture: CaptureStatus::NoneObserved,
    });
}

#[given("rustc classified one argument as Captured")]
fn given_captured_argument(world: &WorldCell) {
    world
        .borrow_mut()
        .set_context(LiteralContext::SourceFormatString {
            capture: CaptureStatus::Present,
        });
}

#[given("a cooked string whose first token after the opening quote is a continuation")]
fn given_leading_continuation(world: &WorldCell) { world.borrow_mut().set_body("\\\n    alpha"); }

#[when("the continuation is classified")]
fn when_classified(world: &WorldCell) { world.borrow_mut().classify(); }

#[then("the disposition is PreferConcat")]
fn then_prefer_concat(world: &WorldCell) {
    assert_eq!(world.borrow().outcome(), Outcome::PreferConcat);
}

#[then("the disposition is RequireContinuation with reason {reason}")]
fn then_require_continuation(world: &WorldCell, reason: String) {
    let expected = match reason.trim_matches('"') {
        "ImplicitFormatCapture" => RequiredReason::ImplicitFormatCapture,
        "LeadingOrTrailingLayoutTrim" => RequiredReason::LeadingOrTrailingLayoutTrim,
        other => panic!("unsupported reason in the feature file: {other}"),
    };
    assert_eq!(
        world.borrow().outcome(),
        Outcome::RequireContinuation(expected)
    );
}

#[scenario(path = "tests/features/string_continuation.feature", index = 0)]
fn scenario_join_prefers_concat(world: WorldCell) { let _ = world; }

#[scenario(path = "tests/features/string_continuation.feature", index = 1)]
fn scenario_capture_requires_continuation(world: WorldCell) { let _ = world; }

#[scenario(path = "tests/features/string_continuation.feature", index = 2)]
fn scenario_leading_continuation_requires_continuation(world: WorldCell) { let _ = world; }
