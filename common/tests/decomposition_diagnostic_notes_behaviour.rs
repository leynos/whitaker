//! Behaviour-driven coverage for decomposition diagnostic-note rendering.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::BTreeMap;
use whitaker_common::decomposition_advice::{
    DecompositionContext, MethodProfileBuilder, SubjectKind, format_diagnostic_note,
    suggest_decomposition,
};
use whitaker_common::test_support::decomposition::{
    parser_serde_fs_fixture, transport_trait_fixture,
};

#[derive(Debug, Clone)]
struct CsvList(Vec<String>);

impl CsvList {
    fn into_vec(self) -> Vec<String> {
        self.0
    }
}

#[derive(Debug, Clone)]
struct QuotedString(String);

impl QuotedString {
    fn into_inner(self) -> String {
        self.0
    }
}

impl std::str::FromStr for CsvList {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let items = s
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Ok(Self(items))
    }
}

impl std::str::FromStr for QuotedString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim_matches('"').to_owned()))
    }
}

#[derive(Debug, Default)]
struct DiagnosticNoteWorld {
    context: RefCell<Option<DecompositionContext>>,
    methods: RefCell<BTreeMap<usize, MethodProfileBuilder>>,
    method_ids_by_name: RefCell<BTreeMap<String, Vec<usize>>>,
    next_method_id: RefCell<usize>,
    render_attempted: RefCell<bool>,
    rendered_note: RefCell<Option<String>>,
}

#[fixture]
fn world() -> DiagnosticNoteWorld {
    DiagnosticNoteWorld::default()
}

fn create_method_builder(world: &DiagnosticNoteWorld, method_name: &str) {
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
    world: &DiagnosticNoteWorld,
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
                .unwrap_or_else(|| panic!("method id must exist after creation"))
        }
    };

    let mut methods = world.methods.borrow_mut();
    let builder = methods
        .get_mut(&method_id)
        .unwrap_or_else(|| panic!("method id {method_id} must exist while applying updates"));
    update(builder);
}

fn with_rendered_note(
    world: &DiagnosticNoteWorld,
    assert_fn: impl FnOnce(&Option<String>) -> Result<(), String>,
) -> Result<(), String> {
    if !*world.render_attempted.borrow() {
        return Err(String::from(
            "note must be rendered before running assertions",
        ));
    }
    let rendered_note = world.rendered_note.borrow();
    assert_fn(&rendered_note)
}

fn seed_methods(world: &DiagnosticNoteWorld, methods: Vec<whitaker_common::MethodProfile>) {
    for method in methods {
        let method_name = method.name().to_owned();
        let method_id = {
            let mut next_method_id = world.next_method_id.borrow_mut();
            let method_id = *next_method_id;
            *next_method_id += 1;
            method_id
        };
        let mut builder = MethodProfileBuilder::new(&method_name);
        for field in method.accessed_fields() {
            builder.record_accessed_field(field);
        }
        for type_name in method.signature_types() {
            builder.record_signature_type(type_name);
        }
        for type_name in method.local_types() {
            builder.record_local_type(type_name);
        }
        for domain in method.external_domains() {
            builder.record_external_domain(domain);
        }
        world.methods.borrow_mut().insert(method_id, builder);
        world
            .method_ids_by_name
            .borrow_mut()
            .entry(method_name)
            .or_default()
            .push(method_id);
    }
}

#[given("note rendering for a {kind} named {name}")]
fn given_context(world: &DiagnosticNoteWorld, kind: SubjectKind, name: String) {
    *world.context.borrow_mut() = Some(DecompositionContext::new(name, kind));
}

#[given("a method named {name}")]
fn given_method(world: &DiagnosticNoteWorld, name: String) {
    create_method_builder(world, &name);
}

#[given("the parser, serde, and filesystem methods are tracked")]
fn given_parser_serde_fs_fixture(world: &DiagnosticNoteWorld) {
    seed_methods(world, parser_serde_fs_fixture());
}

#[given("the transport serde and io methods are tracked")]
fn given_transport_fixture(world: &DiagnosticNoteWorld) {
    seed_methods(world, transport_trait_fixture());
}

#[given("method {name} accesses fields {fields}")]
fn given_fields(world: &DiagnosticNoteWorld, name: String, fields: CsvList) {
    let parsed_fields = fields.into_vec();
    with_method_builder(world, &name, |builder| {
        for field in &parsed_fields {
            builder.record_accessed_field(field.as_str());
        }
    });
}

#[given("method {name} uses external domains {domains}")]
fn given_external_domains(world: &DiagnosticNoteWorld, name: String, domains: CsvList) {
    let parsed_domains = domains.into_vec();
    with_method_builder(world, &name, |builder| {
        for domain in &parsed_domains {
            builder.record_external_domain(domain.as_str());
        }
    });
}

#[when("the decomposition diagnostic note is rendered")]
fn when_note_is_rendered(world: &DiagnosticNoteWorld) -> Result<(), String> {
    let context = world
        .context
        .borrow()
        .clone()
        .ok_or_else(|| String::from("context must be configured before rendering"))?;
    let builders: Vec<_> = world.methods.borrow().values().cloned().collect();
    let profiles = builders
        .into_iter()
        .map(MethodProfileBuilder::build)
        .collect::<Vec<_>>();
    let suggestions = suggest_decomposition(&context, &profiles);
    *world.render_attempted.borrow_mut() = true;
    *world.rendered_note.borrow_mut() = format_diagnostic_note(&context, &suggestions);
    Ok(())
}

#[then("the note is present")]
fn then_note_is_present(world: &DiagnosticNoteWorld) -> Result<(), String> {
    with_rendered_note(world, |rendered_note| match rendered_note {
        Some(_) => Ok(()),
        None => Err(String::from("expected a rendered note but found none")),
    })
}

#[then("there is no note")]
fn then_there_is_no_note(world: &DiagnosticNoteWorld) -> Result<(), String> {
    with_rendered_note(world, |rendered_note| match rendered_note {
        Some(note) => Err(format!("expected no note but found:\n{note}")),
        None => Ok(()),
    })
}

#[then("the note contains line {line}")]
fn then_note_contains_line(world: &DiagnosticNoteWorld, line: QuotedString) -> Result<(), String> {
    let line = line.into_inner();
    with_rendered_note(world, |rendered_note| {
        let note = rendered_note
            .as_ref()
            .ok_or_else(|| String::from("expected a rendered note but found none"))?;
        if note.lines().any(|candidate| candidate == line) {
            Ok(())
        } else {
            Err(format!(
                "expected note to contain line `{line}` but found:\n{note}"
            ))
        }
    })
}

#[then("the note does not contain {fragment}")]
fn then_note_does_not_contain(
    world: &DiagnosticNoteWorld,
    fragment: QuotedString,
) -> Result<(), String> {
    let fragment = fragment.into_inner();
    with_rendered_note(world, |rendered_note| {
        let note = rendered_note
            .as_ref()
            .ok_or_else(|| String::from("expected a rendered note but found none"))?;
        if note.contains(&fragment) {
            Err(format!(
                "expected note not to contain `{fragment}` but found:\n{note}"
            ))
        } else {
            Ok(())
        }
    })
}

// `rstest-bdd` scenario bindings are index-based, so preserve ordering with
// the feature file when editing scenarios.
#[scenario(
    path = "tests/features/decomposition_diagnostic_notes.feature",
    index = 0
)]
fn scenario_type_note_renders_three_areas(world: DiagnosticNoteWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_diagnostic_notes.feature",
    index = 1
)]
fn scenario_trait_note_renders_sub_traits(world: DiagnosticNoteWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_diagnostic_notes.feature",
    index = 2
)]
fn scenario_no_suggestions_yield_no_note(world: DiagnosticNoteWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_diagnostic_notes.feature",
    index = 3
)]
fn scenario_large_subjects_cap_rendered_areas(world: DiagnosticNoteWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_diagnostic_notes.feature",
    index = 4
)]
fn scenario_large_communities_cap_method_names(world: DiagnosticNoteWorld) {
    let _ = world;
}
