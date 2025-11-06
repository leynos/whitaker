use super::{FluentResource, get_all_ftl_files};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[path = "../support/mod.rs"]
mod support;
use support::{extract_identifier, should_skip_line};

#[derive(Default)]
struct DiscoveryWorld {
    paths: RefCell<Vec<PathBuf>>,
}

impl DiscoveryWorld {
    fn collect(&self) {
        let paths = get_all_ftl_files();
        self.paths.borrow_mut().extend(paths);
    }

    fn all_unique(&self) -> bool {
        let paths = self.paths.borrow();
        let mut set = BTreeSet::new();
        paths.iter().all(|path| set.insert(path.clone()))
    }

    fn contains(&self, expected: &str) -> bool {
        let paths = self.paths.borrow();
        paths
            .iter()
            .any(|path| path.to_string_lossy().ends_with(expected))
    }
}

#[fixture]
fn discovery_world() -> DiscoveryWorld {
    DiscoveryWorld::default()
}

#[when("I collect all Fluent files")]
fn when_collect(discovery_world: &DiscoveryWorld) {
    discovery_world.collect();
}

#[then("each Fluent path is unique")]
fn then_unique(discovery_world: &DiscoveryWorld) {
    assert!(discovery_world.all_unique(), "expected unique Fluent files");
}

#[then("the collection includes {path}")]
fn then_contains(discovery_world: &DiscoveryWorld, path: String) {
    assert!(
        discovery_world.contains(&path),
        "expected to find Fluent bundle {:?}",
        path
    );
}

#[derive(Default)]
struct ParsingWorld {
    content: RefCell<Option<String>>,
    outcome: RefCell<Option<Result<(), usize>>>,
}

fn duplicate_message_count(source: &str) -> usize {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut duplicates = 0;

    for identifier in source.lines().filter_map(extract_identifier) {
        if !seen.insert(identifier) {
            duplicates += 1;
        }
    }

    duplicates
}

impl ParsingWorld {
    fn set_fixture(&self, fixture: &str) {
        let template = match fixture {
            "valid" => "message = Value".to_string(),
            "invalid" => "message = {".to_string(),
            "duplicate" => String::from("one = First\none = Second"),
            other => panic!("unknown fixture: {other}"),
        };
        self.content.borrow_mut().replace(template);
    }

    fn parse(&self) {
        let source = self
            .content
            .borrow()
            .as_ref()
            .cloned()
            .expect("Fluent source should be initialised");
        let duplicate_errors = duplicate_message_count(&source);
        let result = match FluentResource::try_new(source) {
            Ok(_) => {
                if duplicate_errors > 0 {
                    Err(duplicate_errors)
                } else {
                    Ok(())
                }
            }
            Err((_, errors)) => Err(errors.len() + duplicate_errors),
        };
        self.outcome.borrow_mut().replace(result);
    }

    fn result(&self) -> Result<(), usize> {
        self.outcome
            .borrow()
            .as_ref()
            .cloned()
            .expect("parse result should be recorded")
    }
}

#[fixture]
fn parsing_world() -> ParsingWorld {
    ParsingWorld::default()
}

#[given("the Fluent resource fixture {fixture}")]
fn given_fixture(parsing_world: &ParsingWorld, fixture: String) {
    parsing_world.set_fixture(&fixture);
}

#[when("I parse the Fluent resource")]
fn when_parse(parsing_world: &ParsingWorld) {
    parsing_world.parse();
}

#[then("the parse succeeds")]
fn then_success(parsing_world: &ParsingWorld) {
    assert!(parsing_world.result().is_ok(), "expected parse success");
}

#[then("the parse fails with {count} errors")]
fn then_failure(parsing_world: &ParsingWorld, count: usize) {
    match parsing_world.result() {
        Ok(_) => panic!("expected parse failure"),
        Err(errors) => assert_eq!(errors, count, "unexpected error count"),
    }
}

#[scenario(path = "tests/features/i18n_ftl_smoke.feature", index = 0)]
fn scenario_collects_files(discovery_world: DiscoveryWorld) {
    let _ = discovery_world;
}

#[scenario(path = "tests/features/i18n_ftl_smoke.feature", index = 1)]
fn scenario_parse_valid(parsing_world: ParsingWorld) {
    let _ = parsing_world;
}

#[scenario(path = "tests/features/i18n_ftl_smoke.feature", index = 2)]
fn scenario_parse_invalid(parsing_world: ParsingWorld) {
    let _ = parsing_world;
}

#[scenario(path = "tests/features/i18n_ftl_smoke.feature", index = 3)]
fn scenario_parse_duplicate(parsing_world: ParsingWorld) {
    let _ = parsing_world;
}
