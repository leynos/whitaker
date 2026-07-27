//! Behaviour-driven tests for module-path scoped suppression.
//!
//! These scenarios exercise the pure `PathExclusions` decision the driver makes
//! for each detected `std::fs` usage, without needing a live `rustc` session.

use crate::exclusion::PathExclusions;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::HashSet;
use whitaker_common::SimplePath;

#[derive(Default)]
struct ExclusionWorld {
    excluded_paths: HashSet<String>,
    suppressed: Option<bool>,
}

impl ExclusionWorld {
    fn exclude_path(&mut self, path: &str) {
        self.excluded_paths.insert(path.to_owned());
    }

    fn evaluate(&mut self, item_path: &str) {
        let exclusions = PathExclusions::new(&self.excluded_paths);
        self.suppressed = Some(exclusions.excludes(&SimplePath::parse(item_path)));
    }

    fn suppressed(&self) -> bool {
        self.suppressed
            .expect("a usage should have been evaluated before asserting")
    }
}

type WorldCell = RefCell<ExclusionWorld>;

#[fixture]
fn world() -> WorldCell {
    RefCell::new(ExclusionWorld::default())
}

#[given("the module path {path} is excluded")]
fn given_excluded_path(world: &WorldCell, path: String) {
    world.borrow_mut().exclude_path(path.trim_matches('"'));
}

#[given("no module paths are excluded")]
fn given_no_excluded_paths(world: &WorldCell) {
    world.borrow_mut().excluded_paths.clear();
}

#[when("a std::fs usage is found in item {item}")]
fn when_usage_found(world: &WorldCell, item: String) {
    world.borrow_mut().evaluate(item.trim_matches('"'));
}

#[then("the usage is suppressed")]
fn then_suppressed(world: &WorldCell) {
    assert!(
        world.borrow().suppressed(),
        "expected the usage to be suppressed"
    );
}

#[then("the usage is reported")]
fn then_reported(world: &WorldCell) {
    assert!(
        !world.borrow().suppressed(),
        "expected the usage to be reported"
    );
}

#[scenario(path = "tests/features/path_exclusion.feature", index = 0)]
fn scenario_nested_child_suppressed(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/path_exclusion.feature", index = 1)]
fn scenario_exact_module_suppressed(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/path_exclusion.feature", index = 2)]
fn scenario_sibling_reported(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/path_exclusion.feature", index = 3)]
fn scenario_unrelated_reported(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/path_exclusion.feature", index = 4)]
fn scenario_crate_root_suppresses_all(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/path_exclusion.feature", index = 5)]
fn scenario_no_exclusions_reports(world: WorldCell) {
    let _ = world;
}
