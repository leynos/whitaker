//! Behaviour-driven coverage for brain trait threshold evaluation.

use common::brain_trait_metrics::evaluation::{
    BrainTraitDiagnostic, BrainTraitDisposition, BrainTraitThresholds, BrainTraitThresholdsBuilder,
    evaluate_brain_trait, format_primary_message,
};
use common::brain_trait_metrics::{TraitMetrics, TraitMetricsBuilder};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};

#[derive(Debug)]
struct EvaluationWorld {
    trait_name: RefCell<String>,
    required_count: Cell<usize>,
    default_count: Cell<usize>,
    associated_type_count: Cell<usize>,
    associated_const_count: Cell<usize>,
    default_cc_sum: Cell<usize>,
    thresholds: RefCell<Option<BrainTraitThresholds>>,
    built_metrics: RefCell<Option<TraitMetrics>>,
    disposition: Cell<Option<BrainTraitDisposition>>,
    primary_message: RefCell<Option<String>>,
}

impl Default for EvaluationWorld {
    fn default() -> Self {
        Self {
            trait_name: RefCell::new(String::from("Unnamed")),
            required_count: Cell::new(0),
            default_count: Cell::new(0),
            associated_type_count: Cell::new(0),
            associated_const_count: Cell::new(0),
            default_cc_sum: Cell::new(0),
            thresholds: RefCell::new(None),
            built_metrics: RefCell::new(None),
            disposition: Cell::new(None),
            primary_message: RefCell::new(None),
        }
    }
}

/// Distributes `cc_sum` across `count` default methods, adding each to
/// `builder`. The remainder is assigned to the last method.
fn add_distributed_defaults(builder: &mut TraitMetricsBuilder, count: usize, cc_sum: usize) {
    let base_cc = cc_sum / count;
    let remainder = cc_sum % count;
    for i in 0..count {
        let cc = base_cc + if i == count - 1 { remainder } else { 0 };
        builder.add_default_method(format!("default_{i}"), cc, false);
    }
}

impl EvaluationWorld {
    /// Builds `TraitMetrics` from the configured world state.
    ///
    /// Required methods are named `req_0`, `req_1`, etc. Default
    /// methods are named `default_0`, `default_1`, etc. with the
    /// `default_cc_sum` distributed evenly (remainder on the last
    /// method). Associated types and consts are added if configured.
    fn build_metrics(&self) -> TraitMetrics {
        let name = self.trait_name.borrow().clone();
        let mut builder = TraitMetricsBuilder::new(name);

        let required_count = self.required_count.get();
        for i in 0..required_count {
            builder.add_required_method(format!("req_{i}"));
        }

        let default_count = self.default_count.get();
        if default_count > 0 {
            add_distributed_defaults(&mut builder, default_count, self.default_cc_sum.get());
        }

        let assoc_types = self.associated_type_count.get();
        for i in 0..assoc_types {
            builder.add_associated_type(format!("Type_{i}"));
        }

        let assoc_consts = self.associated_const_count.get();
        for i in 0..assoc_consts {
            builder.add_associated_const(format!("CONST_{i}"));
        }

        builder.build()
    }
}

#[fixture]
fn world() -> EvaluationWorld {
    EvaluationWorld::default()
}

// --- Given steps ---

#[given("a trait called {name} with {required} required and {default} default methods")]
fn given_trait_methods(world: &EvaluationWorld, name: String, required: usize, default: usize) {
    *world.trait_name.borrow_mut() = name;
    world.required_count.set(required);
    world.default_count.set(default);
}

#[given("{types} associated types and {consts} associated consts")]
fn given_associated_items(world: &EvaluationWorld, types: usize, consts: usize) {
    world.associated_type_count.set(types);
    world.associated_const_count.set(consts);
}

#[given("default method CC sum of {cc_sum}")]
fn given_default_cc_sum(world: &EvaluationWorld, cc_sum: usize) {
    world.default_cc_sum.set(cc_sum);
}

#[given("the default brain trait thresholds")]
fn given_default_thresholds(world: &EvaluationWorld) {
    *world.thresholds.borrow_mut() = Some(BrainTraitThresholdsBuilder::new().build());
}

// --- When steps ---

#[when("brain trait thresholds are evaluated")]
fn when_evaluate(world: &EvaluationWorld) {
    let metrics = world.build_metrics();
    let thresholds = world
        .thresholds
        .borrow()
        .unwrap_or_else(|| BrainTraitThresholdsBuilder::new().build());
    let result = evaluate_brain_trait(&metrics, &thresholds);
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
    let diag = BrainTraitDiagnostic::new(metrics, disposition);
    *world.primary_message.borrow_mut() = Some(format_primary_message(&diag));
}

// --- Then steps ---

#[then("the disposition is pass")]
fn then_disposition_pass(world: &EvaluationWorld) {
    assert_eq!(
        world.disposition.get(),
        Some(BrainTraitDisposition::Pass),
        "expected Pass disposition"
    );
}

#[then("the disposition is warn")]
fn then_disposition_warn(world: &EvaluationWorld) {
    assert_eq!(
        world.disposition.get(),
        Some(BrainTraitDisposition::Warn),
        "expected Warn disposition"
    );
}

#[then("the disposition is deny")]
fn then_disposition_deny(world: &EvaluationWorld) {
    assert_eq!(
        world.disposition.get(),
        Some(BrainTraitDisposition::Deny),
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
// `tests/features/brain_trait_evaluation.feature`. Adding, removing, or
// reordering scenarios in the feature file requires updating the indices
// here.

#[scenario(path = "tests/features/brain_trait_evaluation.feature", index = 0)]
fn scenario_within_limits_passes(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_evaluation.feature", index = 1)]
fn scenario_all_warn_conditions(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_evaluation.feature", index = 2)]
fn scenario_many_methods_alone(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_evaluation.feature", index = 3)]
fn scenario_high_cc_alone(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_evaluation.feature", index = 4)]
fn scenario_deny_threshold(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_evaluation.feature", index = 5)]
fn scenario_deny_supersedes_warn(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_evaluation.feature", index = 6)]
fn scenario_associated_items_excluded(world: EvaluationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_evaluation.feature", index = 7)]
fn scenario_diagnostic_surfaces_values(world: EvaluationWorld) {
    let _ = world;
}
