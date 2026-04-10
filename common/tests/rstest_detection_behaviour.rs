//! Behaviour-driven tests for strict `rstest` detection helpers.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::BTreeSet;
use whitaker_common::attributes::{Attribute, AttributeKind, AttributePath};
use whitaker_common::rstest::{
    ExpansionTrace, ParameterBinding, RstestDetectionOptions, RstestParameter, RstestParameterKind,
    classify_rstest_parameter, fixture_local_names, is_rstest_fixture_with, is_rstest_test_with,
};

#[derive(Clone, Debug, Default)]
struct DetectionWorld {
    attributes: RefCell<Vec<Attribute>>,
    parameters: RefCell<Vec<RstestParameter>>,
    trace: RefCell<Option<ExpansionTrace>>,
    options: RefCell<RstestDetectionOptions>,
    test_result: RefCell<Option<bool>>,
    fixture_result: RefCell<Option<bool>>,
    parameter_kind: RefCell<Option<RstestParameterKind>>,
    fixture_names: RefCell<Option<BTreeSet<String>>>,
}

impl DetectionWorld {
    fn push_attribute(&self, path: &str) {
        self.attributes.borrow_mut().push(Attribute::new(
            AttributePath::from(path),
            AttributeKind::Outer,
        ));
    }

    fn set_parameter(&self, parameter: RstestParameter) {
        let mut parameters = self.parameters.borrow_mut();
        parameters.clear();
        parameters.push(parameter);
    }

    fn set_trace(&self, path: &str) {
        self.trace
            .borrow_mut()
            .replace(ExpansionTrace::new([AttributePath::from(path)]));
    }

    fn set_multi_frame_trace(&self, paths: &[&str]) {
        self.trace.borrow_mut().replace(ExpansionTrace::new(
            paths.iter().map(|p| AttributePath::from(*p)),
        ));
    }

    fn add_custom_provider_attributes(&self, paths: Vec<AttributePath>) {
        let use_trace_fallback = self.options.borrow().use_expansion_trace_fallback();
        self.options
            .replace(RstestDetectionOptions::new(paths, use_trace_fallback));
    }

    fn enable_trace_fallback(&self) {
        let provider_paths = self.options.borrow().provider_param_attributes().to_vec();
        self.options
            .replace(RstestDetectionOptions::new(provider_paths, true));
    }

    fn evaluate_test(&self) {
        let attrs = self.attributes.borrow();
        let trace = self.trace.borrow();
        let options = self.options.borrow();
        self.test_result
            .replace(Some(is_rstest_test_with(&attrs, trace.as_ref(), &options)));
    }

    fn evaluate_fixture(&self) {
        let attrs = self.attributes.borrow();
        let trace = self.trace.borrow();
        let options = self.options.borrow();
        self.fixture_result.replace(Some(is_rstest_fixture_with(
            &attrs,
            trace.as_ref(),
            &options,
        )));
    }

    fn evaluate_parameter(&self) -> Result<(), String> {
        let parameter = self
            .parameters
            .borrow()
            .first()
            .cloned()
            .ok_or("parameter must be configured before classification")?;
        let options = self.options.borrow();
        self.parameter_kind
            .replace(Some(classify_rstest_parameter(&parameter, &options)));
        Ok(())
    }

    fn evaluate_fixture_names(&self) {
        let parameters = self.parameters.borrow();
        let options = self.options.borrow();
        self.fixture_names
            .replace(Some(fixture_local_names(&parameters, &options)));
    }
}

#[fixture]
fn world() -> DetectionWorld {
    DetectionWorld::default()
}

#[given("a function annotated with rstest")]
fn given_rstest_function(world: &DetectionWorld) {
    world.push_attribute("rstest");
}

#[given("a function annotated with rstest::fixture")]
fn given_rstest_fixture(world: &DetectionWorld) {
    world.push_attribute("rstest::fixture");
}

#[given("a parameter named db")]
fn given_fixture_local_parameter(world: &DetectionWorld) {
    world.set_parameter(RstestParameter::ident("db"));
}

#[given("a parameter named case_input annotated with case")]
fn given_provider_parameter(world: &DetectionWorld) {
    world.set_parameter(RstestParameter::new(
        ParameterBinding::Ident("case_input".to_string()),
        vec![Attribute::new(
            AttributePath::from("case"),
            AttributeKind::Outer,
        )],
    ));
}

#[given("a destructured parameter binding")]
fn given_unsupported_parameter(world: &DetectionWorld) {
    world.set_parameter(RstestParameter::unsupported());
}

#[given("the expansion trace contains rstest")]
fn given_trace(world: &DetectionWorld) {
    world.set_trace("rstest");
}

#[given("expansion fallback is enabled")]
fn given_fallback_enabled(world: &DetectionWorld) {
    world.enable_trace_fallback();
}

#[given("a function annotated with rstest and allow")]
fn given_rstest_and_allow(world: &DetectionWorld) {
    world.push_attribute("rstest");
    world.push_attribute("allow");
}

#[given("a parameter annotated with a custom provider attribute")]
fn given_custom_provider_parameter(world: &DetectionWorld) {
    world.set_parameter(RstestParameter::new(
        ParameterBinding::Ident("custom_value".to_string()),
        vec![Attribute::new(
            AttributePath::from("custom::provider"),
            AttributeKind::Outer,
        )],
    ));
}

#[given("custom provider attributes are configured")]
fn given_custom_provider_config(world: &DetectionWorld) {
    world.add_custom_provider_attributes(vec![AttributePath::from("custom::provider")]);
}

#[given("the expansion trace contains outer_macro and rstest")]
fn given_multi_frame_trace(world: &DetectionWorld) {
    world.set_multi_frame_trace(&["outer_macro", "rstest"]);
}

#[when("I check whether the function is an rstest test")]
fn when_check_test(world: &DetectionWorld) {
    world.evaluate_test();
}

#[when("I check whether the function is an rstest fixture")]
fn when_check_fixture(world: &DetectionWorld) {
    world.evaluate_fixture();
}

#[when("I classify the parameter")]
fn when_classify_parameter(world: &DetectionWorld) -> Result<(), String> {
    world.evaluate_parameter()
}

#[when("I evaluate fixture-local names")]
fn when_fixture_names_evaluated(world: &DetectionWorld) {
    world.evaluate_fixture_names();
}

#[then("the function is recognised as an rstest test")]
fn then_test_positive(world: &DetectionWorld) {
    assert_eq!(*world.test_result.borrow(), Some(true));
}

#[then("the function is recognised as not being an rstest test")]
fn then_test_negative(world: &DetectionWorld) {
    assert_eq!(*world.test_result.borrow(), Some(false));
}

#[then("the function is recognised as an rstest fixture")]
fn then_fixture_positive(world: &DetectionWorld) {
    assert_eq!(*world.fixture_result.borrow(), Some(true));
}

#[then("the parameter is classified as fixture-local")]
fn then_fixture_local(world: &DetectionWorld) {
    assert_eq!(
        *world.parameter_kind.borrow(),
        Some(RstestParameterKind::FixtureLocal {
            name: "db".to_string()
        })
    );
}

#[then("the parameter is classified as provider-driven")]
fn then_provider(world: &DetectionWorld) {
    assert_eq!(
        *world.parameter_kind.borrow(),
        Some(RstestParameterKind::Provider)
    );
}

#[then("the parameter is classified as unsupported")]
fn then_unsupported(world: &DetectionWorld) {
    assert_eq!(
        *world.parameter_kind.borrow(),
        Some(RstestParameterKind::UnsupportedPattern)
    );
}

#[then("the fixture-local names contain db")]
fn then_fixture_names(world: &DetectionWorld) {
    assert_eq!(
        *world.fixture_names.borrow(),
        Some(BTreeSet::from(["db".to_string()]))
    );
}

macro_rules! declare_scenarios {
    ($(($index:literal, $name:ident)),+ $(,)?) => {
        $(
            #[scenario(path = "tests/features/rstest_detection.feature", index = $index)]
            fn $name(world: DetectionWorld) {
                let _ = world;
            }
        )+
    };
}

declare_scenarios!(
    (0, scenario_detects_rstest_test),
    (1, scenario_detects_rstest_fixture),
    (2, scenario_classifies_fixture_local_parameter),
    (3, scenario_classifies_provider_parameter),
    (4, scenario_ignores_unsupported_parameter),
    (5, scenario_ignores_trace_without_fallback),
    (6, scenario_uses_trace_with_fallback),
    (7, scenario_detects_rstest_with_multiple_attributes),
    (8, scenario_classifies_custom_provider_parameters),
    (9, scenario_uses_multi_frame_traces),
);
