//! Behaviour-driven coverage for decomposition advice analysis.

use common::decomposition_advice::{
    DecompositionContext, DecompositionSuggestion, MethodProfileBuilder, SubjectKind,
    SuggestedExtractionKind, suggest_decomposition,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct CsvList(Vec<String>);

impl CsvList {
    fn into_vec(self) -> Vec<String> {
        self.0
    }
}

impl std::str::FromStr for CsvList {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let items = s
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Ok(CsvList(items))
    }
}

#[derive(Debug, Default)]
struct DecompositionWorld {
    context: RefCell<Option<DecompositionContext>>,
    methods: RefCell<BTreeMap<usize, MethodProfileBuilder>>,
    method_ids_by_name: RefCell<BTreeMap<String, Vec<usize>>>,
    next_method_id: RefCell<usize>,
    suggestions: RefCell<Option<Vec<DecompositionSuggestion>>>,
}

#[fixture]
fn world() -> DecompositionWorld {
    DecompositionWorld::default()
}

fn create_method_builder(world: &DecompositionWorld, method_name: &str) {
    let mut next_method_id = world.next_method_id.borrow_mut();
    let method_id = *next_method_id;
    *next_method_id += 1;

    world
        .methods
        .borrow_mut()
        .insert(method_id, MethodProfileBuilder::new(method_name));
    world
        .method_ids_by_name
        .borrow_mut()
        .entry(method_name.to_owned())
        .or_default()
        .push(method_id);
}

fn with_method_builder(
    world: &DecompositionWorld,
    method_name: &str,
    update: impl FnOnce(&mut MethodProfileBuilder),
) {
    let method_id = world
        .method_ids_by_name
        .borrow()
        .get(method_name)
        .and_then(|ids| ids.last().copied());

    let method_id = match method_id {
        Some(method_id) => method_id,
        None => {
            create_method_builder(world, method_name);
            world
                .method_ids_by_name
                .borrow()
                .get(method_name)
                .and_then(|ids| ids.last().copied())
                .unwrap_or_else(|| unreachable!("method id must exist after creation"))
        }
    };

    let mut methods = world.methods.borrow_mut();
    let Some(builder) = methods.get_mut(&method_id) else {
        panic!("method id {method_id} must exist while applying updates");
    };
    update(builder);
}

fn with_suggestions(
    world: &DecompositionWorld,
    assert_fn: impl FnOnce(&[DecompositionSuggestion]) -> Result<(), String>,
) -> Result<(), String> {
    let suggestions = world.suggestions.borrow();
    let Some(suggestions) = suggestions.as_ref() else {
        return Err(String::from(
            "suggestions must be generated before running assertions",
        ));
    };
    assert_fn(suggestions)
}

#[test]
fn csv_list_trims_values() {
    let values: CsvList = "a, b ,c".parse().unwrap_or_else(|never| match never {});
    assert_eq!(values.into_vec(), ["a", "b", "c"]);
}

#[test]
fn csv_list_handles_empty_and_extra_commas() {
    let empty: CsvList = "".parse().unwrap_or_else(|never| match never {});
    assert!(empty.into_vec().is_empty());

    let values: CsvList = ",a,,b,".parse().unwrap_or_else(|never| match never {});
    assert_eq!(values.into_vec(), ["a", "b"]);
}

#[test]
fn duplicate_method_names_use_distinct_builders() {
    let world = DecompositionWorld::default();

    create_method_builder(&world, "load");
    create_method_builder(&world, "load");

    with_method_builder(&world, "load", |builder| {
        builder.record_external_domain("serde::json");
    });

    let builders: Vec<_> = world.methods.borrow().values().cloned().collect();
    let profiles = builders
        .into_iter()
        .map(MethodProfileBuilder::build)
        .collect::<Vec<_>>();

    assert_eq!(profiles.len(), 2);
    assert_eq!(
        profiles
            .iter()
            .filter(|profile| profile.external_domains().contains("serde::json"))
            .count(),
        1
    );
}

#[given("decomposition analysis for a {kind} named {name}")]
fn given_context(
    world: &DecompositionWorld,
    kind: SubjectKind,
    name: String,
) -> Result<(), String> {
    *world.context.borrow_mut() = Some(DecompositionContext::new(name, kind));
    Ok(())
}

#[given("a method named {name}")]
fn given_method(world: &DecompositionWorld, name: String) {
    create_method_builder(world, &name);
}

#[given("method {name} accesses fields {fields}")]
fn given_fields(world: &DecompositionWorld, name: String, fields: CsvList) {
    let parsed_fields = fields.into_vec();
    with_method_builder(world, &name, |builder| {
        for field in &parsed_fields {
            builder.record_accessed_field(field.as_str());
        }
    });
}

#[given("method {name} uses signature types {types}")]
fn given_signature_types(world: &DecompositionWorld, name: String, types: CsvList) {
    let parsed_types = types.into_vec();
    with_method_builder(world, &name, |builder| {
        for type_name in &parsed_types {
            builder.record_signature_type(type_name.as_str());
        }
    });
}

#[given("method {name} uses local types {types}")]
fn given_local_types(world: &DecompositionWorld, name: String, types: CsvList) {
    let parsed_types = types.into_vec();
    with_method_builder(world, &name, |builder| {
        for type_name in &parsed_types {
            builder.record_local_type(type_name.as_str());
        }
    });
}

#[given("method {name} uses external domains {domains}")]
fn given_external_domains(world: &DecompositionWorld, name: String, domains: CsvList) {
    let parsed_domains = domains.into_vec();
    with_method_builder(world, &name, |builder| {
        for domain in &parsed_domains {
            builder.record_external_domain(domain.as_str());
        }
    });
}

#[when("decomposition suggestions are generated")]
fn when_suggestions_are_generated(world: &DecompositionWorld) -> Result<(), String> {
    let context = world
        .context
        .borrow()
        .clone()
        .ok_or_else(|| String::from("context must be configured before analysis"))?;
    let builders: Vec<_> = world.methods.borrow().values().cloned().collect();
    let profiles = builders
        .into_iter()
        .map(MethodProfileBuilder::build)
        .collect::<Vec<_>>();
    *world.suggestions.borrow_mut() = Some(suggest_decomposition(&context, &profiles));
    Ok(())
}

#[then("suggestion count is {count}")]
fn then_suggestion_count(world: &DecompositionWorld, count: usize) -> Result<(), String> {
    with_suggestions(world, |suggestions| {
        if suggestions.len() == count {
            Ok(())
        } else {
            Err(format!(
                "expected {count} suggestions but found {}",
                suggestions.len()
            ))
        }
    })
}

#[then("there is no suggestion labelled {label}")]
fn then_no_suggestion_label(world: &DecompositionWorld, label: String) -> Result<(), String> {
    with_suggestions(world, |suggestions| {
        if suggestions
            .iter()
            .all(|suggestion| suggestion.label() != label)
        {
            Ok(())
        } else {
            Err(format!("did not expect suggestion labelled {label}"))
        }
    })
}

#[then("there is a {kind} suggestion labelled {label} containing methods {methods}")]
fn then_matching_suggestion(
    world: &DecompositionWorld,
    kind: SuggestedExtractionKind,
    label: String,
    methods: CsvList,
) -> Result<(), String> {
    let expected_methods = methods.into_vec();

    with_suggestions(world, |suggestions| {
        let matches = suggestions.iter().any(|suggestion| {
            suggestion.label() == label
                && suggestion.extraction_kind() == kind
                && suggestion.methods() == expected_methods
        });

        if matches {
            Ok(())
        } else {
            let actual = suggestions
                .iter()
                .map(|s| format!("{}:{}:{:?}", s.label(), s.extraction_kind(), s.methods()))
                .collect::<Vec<_>>();
            Err(format!(
                "missing {kind} suggestion labelled {label} containing methods {:?}; actual suggestions: {:?}",
                expected_methods, actual
            ))
        }
    })
}

#[then("suggestion {label} has rationale {rationale}")]
fn then_suggestion_has_rationale(
    world: &DecompositionWorld,
    label: String,
    rationale: CsvList,
) -> Result<(), String> {
    let expected = rationale.into_vec();

    with_suggestions(world, |suggestions| {
        let suggestion = suggestions
            .iter()
            .find(|suggestion| suggestion.label() == label)
            .ok_or_else(|| format!("missing suggestion labelled {label}"))?;

        if suggestion.rationale() == expected {
            Ok(())
        } else {
            Err(format!(
                "suggestion {label} rationale mismatch: expected {:?}, found {:?}",
                expected,
                suggestion.rationale()
            ))
        }
    })
}

// Scenario indices must match declaration order in the
// `tests/features/decomposition_advice.feature` file.

#[scenario(path = "tests/features/decomposition_advice.feature", index = 0)]
fn scenario_type_method_groups(world: DecompositionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/decomposition_advice.feature", index = 1)]
fn scenario_trait_sub_traits(world: DecompositionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/decomposition_advice.feature", index = 2)]
fn scenario_no_suggestions(world: DecompositionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/decomposition_advice.feature", index = 3)]
fn scenario_singleton_noise(world: DecompositionWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/decomposition_advice.feature", index = 4)]
fn scenario_local_type_groups(world: DecompositionWorld) {
    let _ = world;
}
