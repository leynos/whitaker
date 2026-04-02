//! Behaviour-driven coverage for brain type threshold evaluation.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};
use whitaker_common::brain_type_metrics::evaluation::{
    BrainTypeDiagnostic, BrainTypeDisposition, BrainTypeThresholds, BrainTypeThresholdsBuilder,
    evaluate_brain_type, format_primary_message,
};
use whitaker_common::brain_type_metrics::{MethodMetrics, TypeMetrics, TypeMetricsBuilder};

#[derive(Debug)]
struct EvaluationWorld {
    type_name: RefCell<String>,
    target_wmc: Cell<usize>,
    brain_count: Cell<usize>,
    lcom4: Cell<usize>,
    explicit_brain_methods: RefCell<Vec<MethodMetrics>>,
    thresholds: RefCell<Option<BrainTypeThresholds>>,
    built_metrics: RefCell<Option<TypeMetrics>>,
    disposition: Cell<Option<BrainTypeDisposition>>,
    primary_message: RefCell<Option<String>>,
}

impl Default for EvaluationWorld {
    fn default() -> Self {
        Self {
            type_name: RefCell::new(String::from("Unnamed")),
            target_wmc: Cell::new(0),
            brain_count: Cell::new(0),
            lcom4: Cell::new(0),
            explicit_brain_methods: RefCell::new(Vec::new()),
            thresholds: RefCell::new(None),
            built_metrics: RefCell::new(None),
            disposition: Cell::new(None),
            primary_message: RefCell::new(None),
        }
    }
}

impl EvaluationWorld {
    /// Builds `TypeMetrics` from the configured world state.
    ///
    /// Brain methods are synthesized with CC=30, LOC=100 (above default
    /// thresholds). If explicit brain methods have been added via the
    /// `given_explicit_brain_method` step, those are used instead and the
    /// `brain_count` field is ignored.
    fn build_metrics(&self) -> TypeMetrics {
        let cc_threshold = 25;
        let loc_threshold = 80;
        let brain_cc = 30;
        let brain_loc = 100;
        let name = self.type_name.borrow().clone();
        let mut builder = TypeMetricsBuilder::new(name, cc_threshold, loc_threshold);

        let explicit = self.explicit_brain_methods.borrow();
        let brain_count = self.brain_count.get();
        let target_wmc = self.target_wmc.get();

        if explicit.is_empty() {
            // Synthesize brain methods.
            let brain_total_cc = brain_count * brain_cc;
            for i in 0..brain_count {
                builder.add_method(format!("brain_{i}"), brain_cc, brain_loc);
            }
            if target_wmc > brain_total_cc {
                let filler_cc = target_wmc - brain_total_cc;
                builder.add_method("filler", filler_cc, 10);
            }
        } else {
            // Use explicit brain methods plus filler for remaining WMC.
            let mut explicit_cc: usize = 0;
            for m in explicit.iter() {
                builder.add_method(m.name(), m.cognitive_complexity(), m.lines_of_code());
                explicit_cc += m.cognitive_complexity();
            }
            if target_wmc > explicit_cc {
                let filler_cc = target_wmc - explicit_cc;
                builder.add_method("filler", filler_cc, 10);
            }
        }

        builder.set_lcom4(self.lcom4.get());
        builder.build()
    }
}

#[fixture]
fn world() -> EvaluationWorld {
    EvaluationWorld::default()
}

// --- Given steps ---

#[given("a type called {name} with WMC {wmc} and {brain_count} brain methods")]
fn given_type_metrics(world: &EvaluationWorld, name: String, wmc: usize, brain_count: usize) {
    *world.type_name.borrow_mut() = name;
    world.target_wmc.set(wmc);
    world.brain_count.set(brain_count);
}

#[given("the type has LCOM4 {lcom4}")]
fn given_lcom4(world: &EvaluationWorld, lcom4: usize) {
    world.lcom4.set(lcom4);
}

#[given("the default brain type thresholds")]
fn given_default_thresholds(world: &EvaluationWorld) {
    *world.thresholds.borrow_mut() = Some(BrainTypeThresholdsBuilder::new().build());
}

#[given("a brain method called {name} with CC {cc} and LOC {loc}")]
fn given_explicit_brain_method(world: &EvaluationWorld, name: String, cc: usize, loc: usize) {
    world
        .explicit_brain_methods
        .borrow_mut()
        .push(MethodMetrics::new(name, cc, loc));
}

// --- When steps ---

#[when("brain type thresholds are evaluated")]
fn when_evaluate(world: &EvaluationWorld) {
    let metrics = world.build_metrics();
    let thresholds = world
        .thresholds
        .borrow()
        .unwrap_or_else(|| BrainTypeThresholdsBuilder::new().build());
    let result = evaluate_brain_type(&metrics, &thresholds);
    world.disposition.set(Some(result));
    *world.built_metrics.borrow_mut() = Some(metrics);
}

#[when("the diagnostic message is formatted")]
#[expect(
    clippy::expect_used,
    reason = "metrics and disposition are required for this behaviour test"
)]
fn when_format_diagnostic(world: &EvaluationWorld) {
    let metrics_ref = world.built_metrics.borrow();
    let metrics = metrics_ref
        .as_ref()
        .expect("metrics must be built before formatting");
    let disposition = world
        .disposition
        .get()
        .expect("disposition must be evaluated first");
    let diag = BrainTypeDiagnostic::new(metrics, disposition);
    *world.primary_message.borrow_mut() = Some(format_primary_message(&diag));
}

// --- Then steps ---

#[then("the disposition is pass")]
fn then_disposition_pass(world: &EvaluationWorld) {
    assert_eq!(
        world.disposition.get(),
        Some(BrainTypeDisposition::Pass),
        "expected Pass disposition"
    );
}

#[then("the disposition is warn")]
fn then_disposition_warn(world: &EvaluationWorld) {
    assert_eq!(
        world.disposition.get(),
        Some(BrainTypeDisposition::Warn),
        "expected Warn disposition"
    );
}

#[then("the disposition is deny")]
fn then_disposition_deny(world: &EvaluationWorld) {
    assert_eq!(
        world.disposition.get(),
        Some(BrainTypeDisposition::Deny),
        "expected Deny disposition"
    );
}

#[then("the primary message contains {text}")]
#[expect(
    clippy::expect_used,
    reason = "primary message is required for this behaviour test"
)]
fn then_primary_message_contains(world: &EvaluationWorld, text: String) {
    let msg = world.primary_message.borrow();
    let msg = msg
        .as_deref()
        .expect("primary message must be formatted first");
    assert!(
        msg.contains(&text),
        "expected primary message to contain '{text}', got: {msg}"
    );
}

// Scenario indices must match their declaration order in
// `tests/features/brain_type_evaluation.feature`. Adding, removing, or
// reordering scenarios in the feature file requires updating the indices
// here.

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 0)]
fn scenario_within_limits_passes(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 1)]
fn scenario_all_warn_conditions(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 2)]
fn scenario_high_wmc_alone(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 3)]
fn scenario_brain_method_without_wmc(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 4)]
fn scenario_wmc_deny_threshold(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 5)]
fn scenario_multiple_brain_methods_deny(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 6)]
fn scenario_high_lcom4_deny(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 7)]
fn scenario_deny_supersedes_warn(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_evaluation.feature", index = 8)]
fn scenario_diagnostic_surfaces_values(world: EvaluationWorld) {
    let _ = world;
}
