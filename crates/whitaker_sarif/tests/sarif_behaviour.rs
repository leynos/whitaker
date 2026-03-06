//! Behaviour-driven coverage for SARIF model construction and merge.

mod test_helpers;

use std::cell::RefCell;

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_sarif::{
    Level, LocationBuilder, RegionBuilder, ResultBuilder, RunBuilder, SarifLog, SarifLogBuilder,
    SarifResult, WhitakerProperties, WhitakerPropertiesBuilder, all_rules, merge_runs,
    token_pass_path,
};

use whitaker_sarif::model::descriptor::ReportingDescriptor;
use whitaker_sarif::model::run::Run;

#[derive(Debug, Default)]
struct SarifWorld {
    pending_run: RefCell<Option<Run>>,
    built_log: RefCell<Option<SarifLog>>,
    built_result: RefCell<Option<SarifResult>>,
    result_builder: RefCell<Option<ResultBuilder>>,
    props_builder: RefCell<Option<WhitakerPropertiesBuilder>>,
    props_json: RefCell<Option<serde_json::Value>>,
    runs_to_merge: RefCell<Vec<Run>>,
    merged_run: RefCell<Option<Run>>,
    serialized_json: RefCell<Option<String>>,
    deserialized_log: RefCell<Option<SarifLog>>,
    rules: RefCell<Vec<ReportingDescriptor>>,
    target_dir: RefCell<Option<Utf8PathBuf>>,
    computed_path: RefCell<Option<Utf8PathBuf>>,
}

#[fixture]
fn world() -> SarifWorld {
    SarifWorld::default()
}

// -- Helper functions (match-based to avoid expect/unwrap) --

fn with_log(world: &SarifWorld, assert_fn: impl FnOnce(&SarifLog)) {
    let log = world.built_log.borrow();
    match log.as_ref() {
        Some(log) => assert_fn(log),
        None => panic!("log must be built before running assertions"),
    }
}

fn with_result(world: &SarifWorld, assert_fn: impl FnOnce(&SarifResult)) {
    let result = world.built_result.borrow();
    match result.as_ref() {
        Some(result) => assert_fn(result),
        None => panic!("result must be built before running assertions"),
    }
}

fn with_props_json(world: &SarifWorld, assert_fn: impl FnOnce(&serde_json::Value)) {
    let json = world.props_json.borrow();
    match json.as_ref() {
        Some(json) => assert_fn(json),
        None => panic!("properties JSON must exist before running assertions"),
    }
}

fn with_merged_run(world: &SarifWorld, assert_fn: impl FnOnce(&Run)) {
    let merged = world.merged_run.borrow();
    match merged.as_ref() {
        Some(merged) => assert_fn(merged),
        None => panic!("merged run must exist before running assertions"),
    }
}

fn with_serialized_json(world: &SarifWorld, assert_fn: impl FnOnce(&str)) {
    let json = world.serialized_json.borrow();
    match json.as_ref() {
        Some(json) => assert_fn(json),
        None => panic!("serialized JSON must exist before running assertions"),
    }
}

fn with_computed_path(world: &SarifWorld, assert_fn: impl FnOnce(&Utf8PathBuf)) {
    let path = world.computed_path.borrow();
    match path.as_ref() {
        Some(path) => assert_fn(path),
        None => panic!("computed path must exist before running assertions"),
    }
}

// -- Given steps --

#[given("a run for tool {tool} version {version}")]
fn given_run_for_tool(world: &SarifWorld, tool: String, version: String) {
    let run = RunBuilder::new(tool, version).build();
    *world.pending_run.borrow_mut() = Some(run);
}

#[given("a result for rule {rule}")]
fn given_result_for_rule(world: &SarifWorld, rule: String) {
    *world.result_builder.borrow_mut() = Some(ResultBuilder::new(rule));
}

#[given("the result message is {msg}")]
fn given_result_message(world: &SarifWorld, msg: String) {
    let builder = world.result_builder.borrow_mut().take();
    *world.result_builder.borrow_mut() = builder.map(|b| b.with_message(msg));
}

#[given("a location at file {file} line {line}")]
fn given_location(world: &SarifWorld, file: String, line: usize) {
    let region = match RegionBuilder::new(line).build() {
        Ok(r) => r,
        Err(e) => panic!("failed to build region: {e}"),
    };
    let loc = LocationBuilder::new(file).with_region(region).build();
    let builder = world.result_builder.borrow_mut().take();
    *world.result_builder.borrow_mut() = builder.map(|b| b.with_location(loc));
}

#[given("Whitaker properties with profile {profile}")]
fn given_whitaker_properties(world: &SarifWorld, profile: String) {
    *world.props_builder.borrow_mut() = Some(WhitakerPropertiesBuilder::new(profile));
}

#[given("k value {k}")]
fn given_k_value(world: &SarifWorld, k: usize) {
    let builder = world.props_builder.borrow_mut().take();
    *world.props_builder.borrow_mut() = builder.map(|b| b.with_k(k));
}

#[given("window value {window}")]
fn given_window_value(world: &SarifWorld, window: usize) {
    let builder = world.props_builder.borrow_mut().take();
    *world.props_builder.borrow_mut() = builder.map(|b| b.with_window(window));
}

#[given("a run containing 2 unique results")]
fn given_run_with_two_results(world: &SarifWorld) {
    let r1 = test_helpers::make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
    let r2 = test_helpers::make_keyed_result("WHK002", "src/b.rs", 20, "fp2");
    let run = RunBuilder::new("tool", "1.0")
        .with_result(r1)
        .with_result(r2)
        .build();
    world.runs_to_merge.borrow_mut().push(run);
}

#[given("another run containing 1 duplicate result")]
fn given_run_with_duplicate(world: &SarifWorld) {
    let r1 = test_helpers::make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
    let run = RunBuilder::new("tool", "1.0").with_result(r1).build();
    world.runs_to_merge.borrow_mut().push(run);
}

#[given("a SARIF log with one run and two results")]
fn given_log_with_results(world: &SarifWorld) {
    let r1 = test_helpers::make_keyed_result("WHK001", "src/a.rs", 10, "fp1");
    let r2 = test_helpers::make_keyed_result("WHK002", "src/b.rs", 20, "fp2");
    let run = RunBuilder::new("tool", "1.0")
        .with_result(r1)
        .with_result(r2)
        .build();
    let log = SarifLogBuilder::new().with_run(run).build();
    *world.built_log.borrow_mut() = Some(log);
}

#[given("a SARIF log with no runs")]
fn given_empty_log(world: &SarifWorld) {
    *world.built_log.borrow_mut() = Some(SarifLogBuilder::new().build());
}

#[given("a target directory at {path}")]
fn given_target_dir(world: &SarifWorld, path: String) {
    *world.target_dir.borrow_mut() = Some(Utf8PathBuf::from(path));
}

// -- When steps --

#[when("the SARIF log is built with that run")]
fn when_log_built_with_run(world: &SarifWorld) {
    let run = world.pending_run.borrow_mut().take();
    if let Some(run) = run {
        let log = SarifLogBuilder::new().with_run(run).build();
        *world.built_log.borrow_mut() = Some(log);
    }
}

#[when("the result is built")]
fn when_result_built(world: &SarifWorld) {
    let builder = world.result_builder.borrow_mut().take();
    if let Some(builder) = builder {
        match builder.build() {
            Ok(result) => *world.built_result.borrow_mut() = Some(result),
            Err(e) => panic!("failed to build result: {e}"),
        }
    }
}

#[when("properties are converted to JSON")]
fn when_properties_to_json(world: &SarifWorld) {
    let builder = world.props_builder.borrow_mut().take();
    if let Some(builder) = builder {
        match builder.build() {
            Ok(props) => match props.try_to_value() {
                Ok(value) => *world.props_json.borrow_mut() = Some(value),
                Err(e) => panic!("failed to convert properties to JSON: {e}"),
            },
            Err(e) => panic!("failed to build properties: {e}"),
        }
    }
}

#[when("the runs are merged")]
fn when_runs_merged(world: &SarifWorld) {
    let runs = world.runs_to_merge.borrow();
    match merge_runs(&runs) {
        Ok(merged) => *world.merged_run.borrow_mut() = Some(merged),
        Err(e) => panic!("failed to merge runs: {e}"),
    }
}

#[when("the log is serialized to JSON")]
fn when_log_serialized(world: &SarifWorld) {
    let log = world.built_log.borrow();
    if let Some(log) = log.as_ref() {
        match serde_json::to_string_pretty(log) {
            Ok(json) => *world.serialized_json.borrow_mut() = Some(json),
            Err(e) => panic!("failed to serialize log: {e}"),
        }
    }
}

#[when("the JSON is deserialized back")]
fn when_json_deserialized(world: &SarifWorld) {
    let json = world.serialized_json.borrow();
    if let Some(json) = json.as_ref() {
        match serde_json::from_str::<SarifLog>(json) {
            Ok(log) => *world.deserialized_log.borrow_mut() = Some(log),
            Err(e) => panic!("failed to deserialize log: {e}"),
        }
    }
}

#[when("all Whitaker rules are retrieved")]
fn when_rules_retrieved(world: &SarifWorld) {
    *world.rules.borrow_mut() = all_rules();
}

#[when("the token pass path is requested")]
fn when_token_path_requested(world: &SarifWorld) {
    let dir = world.target_dir.borrow();
    if let Some(dir) = dir.as_ref() {
        let path = token_pass_path(dir);
        *world.computed_path.borrow_mut() = Some(path);
    }
}

// -- Then steps --

#[then("the log version is {version}")]
fn then_log_version(world: &SarifWorld, version: String) {
    with_log(world, |log| assert_eq!(log.version, version));
}

#[then("the log has {count} run")]
fn then_log_has_runs(world: &SarifWorld, count: usize) {
    with_log(world, |log| assert_eq!(log.runs.len(), count));
}

#[then("the run tool name is {name}")]
fn then_run_tool_name(world: &SarifWorld, name: String) {
    with_log(world, |log| match log.runs.first() {
        Some(run) => assert_eq!(run.tool.driver.name, name),
        None => panic!("log must have at least one run"),
    });
}

#[then("the result rule ID is {rule_id}")]
fn then_result_rule_id(world: &SarifWorld, rule_id: String) {
    with_result(world, |result| assert_eq!(result.rule_id, rule_id));
}

#[then("the result level is warning")]
fn then_result_level_warning(world: &SarifWorld) {
    with_result(world, |result| assert_eq!(result.level, Level::Warning));
}

#[then("the result has {count} location")]
fn then_result_location_count(world: &SarifWorld, count: usize) {
    with_result(world, |result| assert_eq!(result.locations.len(), count));
}

#[then("the JSON contains whitaker profile {profile}")]
fn then_json_has_profile(world: &SarifWorld, profile: String) {
    with_props_json(world, |json| match WhitakerProperties::try_from(json) {
        Ok(extracted) => assert_eq!(extracted.profile, profile),
        Err(e) => panic!("failed to extract WhitakerProperties: {e}"),
    });
}

#[then("the JSON contains whitaker k {k}")]
fn then_json_has_k(world: &SarifWorld, k: usize) {
    with_props_json(world, |json| match WhitakerProperties::try_from(json) {
        Ok(extracted) => assert_eq!(extracted.k, k),
        Err(e) => panic!("failed to extract WhitakerProperties: {e}"),
    });
}

#[then("the merged run has {count} results")]
fn then_merged_run_results(world: &SarifWorld, count: usize) {
    with_merged_run(world, |merged| assert_eq!(merged.results.len(), count));
}

#[then("the deserialized log equals the original")]
fn then_deserialized_equals_original(world: &SarifWorld) {
    let original = world.built_log.borrow();
    let deserialized = world.deserialized_log.borrow();
    match (original.as_ref(), deserialized.as_ref()) {
        (Some(orig), Some(deser)) => assert_eq!(orig, deser),
        _ => panic!("both original and deserialized logs must exist"),
    }
}

#[then("the JSON contains version {version}")]
fn then_json_has_version(world: &SarifWorld, version: String) {
    with_serialized_json(world, |json| {
        assert!(json.contains(&format!("\"version\": \"{version}\"")));
    });
}

#[then("there are {count} rules")]
fn then_rule_count(world: &SarifWorld, count: usize) {
    assert_eq!(world.rules.borrow().len(), count);
}

#[then("rule {id} exists")]
fn then_rule_exists(world: &SarifWorld, id: String) {
    let rules = world.rules.borrow();
    assert!(rules.iter().any(|r| r.id == id), "rule {id} not found");
}

#[then("the path ends with {suffix}")]
fn then_path_ends_with(world: &SarifWorld, suffix: String) {
    with_computed_path(world, |path| {
        assert!(
            path.as_str().ends_with(&suffix),
            "expected path to end with '{suffix}', got '{path}'"
        );
    });
}

// -- Scenario bindings (indices match feature file order) --

#[scenario(path = "tests/features/sarif.feature", index = 0)]
fn scenario_minimal_log(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 1)]
fn scenario_result_with_rule(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 2)]
fn scenario_whitaker_properties(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 3)]
fn scenario_merge_deduplicates(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 4)]
fn scenario_round_trip(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 5)]
fn scenario_empty_log(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 6)]
fn scenario_all_rules(world: SarifWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/sarif.feature", index = 7)]
fn scenario_path_helpers(world: SarifWorld) {
    let _ = world;
}
