//! Behaviour-driven tests for the lint crate template helpers.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use toml::Value;
use whitaker::lints::{LintCrateTemplate, TemplateError, TemplateFiles};

#[derive(Debug, Default)]
struct TemplateWorld {
    crate_name: RefCell<String>,
    ui_directory: RefCell<String>,
    lint_constant: RefCell<Option<String>>,
    outcome: RefCell<Option<Result<TemplateFiles, TemplateError>>>,
}

impl TemplateWorld {
    fn set_crate_name(&self, value: String) {
        *self.crate_name.borrow_mut() = value;
    }

    fn crate_name(&self) -> String {
        self.crate_name.borrow().clone()
    }

    fn set_ui_directory(&self, value: String) {
        *self.ui_directory.borrow_mut() = value;
    }

    fn render(&self) {
        let crate_name = self.crate_name.borrow().clone();
        let ui_directory = self.ui_directory.borrow().clone();
        let result =
            LintCrateTemplate::with_ui_tests_directory(crate_name, ui_directory).map(|template| {
                self.lint_constant
                    .borrow_mut()
                    .replace(template.lint_constant().to_string());
                template.render()
            });
        self.outcome.borrow_mut().replace(result);
    }

    fn files(&self) -> TemplateFiles {
        match self.outcome.borrow().as_ref() {
            Some(Ok(files)) => files.clone(),
            Some(Err(error)) => panic!("expected success but observed error: {error}"),
            None => panic!("the template should have been rendered"),
        }
    }

    fn error(&self) -> TemplateError {
        match self.outcome.borrow().as_ref() {
            Some(Err(error)) => error.clone(),
            Some(Ok(_)) => panic!("expected an error but template rendering succeeded"),
            None => panic!("the template should have been rendered"),
        }
    }
}

#[derive(Debug, Clone)]
struct StepString(String);

impl StepString {
    fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for StepString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<StepString> for String {
    fn from(value: StepString) -> Self {
        value.0
    }
}

impl std::str::FromStr for StepString {
    type Err = std::convert::Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(Self(input.to_string()))
    }
}

#[fixture]
fn world() -> TemplateWorld {
    let world = TemplateWorld::default();
    world.set_crate_name("function_attrs_follow_docs".to_string());
    world.set_ui_directory("ui".to_string());
    world
}

#[given("the lint crate name is blank")]
fn given_blank_name(world: &TemplateWorld) {
    world.set_crate_name(String::new());
}

#[given("the lint crate name is {name}")]
fn given_crate_name(world: &TemplateWorld, name: StepString) {
    world.set_crate_name(name.into_inner());
}

#[given("the UI tests directory is {directory}")]
fn given_ui_directory(world: &TemplateWorld, directory: StepString) {
    world.set_ui_directory(directory.into_inner());
}

#[when("I render the lint crate template")]
fn when_render(world: &TemplateWorld) {
    world.render();
}

#[then("the manifest declares a cdylib crate type")]
fn then_manifest_declares_cdylib(world: &TemplateWorld) {
    let files = world.files();
    let document: Value = match files.manifest_document() {
        Ok(document) => document,
        Err(error) => panic!("generated manifest should parse: {error}"),
    };
    let crate_types = document
        .get("lib")
        .and_then(Value::as_table)
        .and_then(|lib| lib.get("crate-type"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    assert!(
        crate_types
            .iter()
            .any(|value| value.as_str() == Some("cdylib"))
    );
}

#[then("the manifest reuses shared dependencies")]
fn then_manifest_reuses_shared_dependencies(world: &TemplateWorld) {
    let files = world.files();
    let document: Value = match files.manifest_document() {
        Ok(document) => document,
        Err(error) => panic!("generated manifest should parse: {error}"),
    };
    let Some(dependencies) = document.get("dependencies").and_then(Value::as_table) else {
        panic!("dependencies table should exist");
    };

    let dylint = dependencies
        .get("dylint_linting")
        .and_then(Value::as_table)
        .and_then(|table| table.get("workspace"))
        .and_then(Value::as_bool)
        .unwrap_or_default();

    assert!(dylint, "dylint_linting should use the workspace dependency");

    let common = dependencies
        .get("common")
        .and_then(Value::as_table)
        .and_then(|table| table.get("path"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    assert_eq!(common, "../../common");

    let Some(dev_dependencies) = document.get("dev-dependencies").and_then(Value::as_table) else {
        panic!("dev-dependencies table should exist");
    };

    let Some(whitaker) = dev_dependencies.get("whitaker").and_then(Value::as_table) else {
        panic!("whitaker dev-dependency should exist");
    };

    let dev_path = whitaker
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default();

    assert_eq!(dev_path, "../../");
}

#[then("the library includes UI test harness boilerplate for directory {directory}")]
fn then_library_includes_harness(world: &TemplateWorld, directory: StepString) {
    let files = world.files();
    let directory = directory.into_inner();
    let expected = format!("whitaker::declare_ui_tests!(\"{directory}\");");
    assert!(files.lib_rs().contains(expected.as_str()));
}

#[then("the lint constant is named {expected}")]
fn then_lint_constant(world: &TemplateWorld, expected: StepString) {
    let Some(lint_constant) = world.lint_constant.borrow().clone() else {
        panic!("lint constant should be stored on success");
    };
    assert_eq!(lint_constant, expected.into_inner());
}

#[then("template creation fails with an empty crate name error")]
fn then_empty_name_error(world: &TemplateWorld) {
    let error = world.error();
    assert_eq!(error, TemplateError::EmptyCrateName);
}

#[then("template creation fails with an invalid crate name character {character}")]
fn then_invalid_character_error(world: &TemplateWorld, character: StepString) {
    let error = world.error();
    let StepString(character) = character;
    let Some(char_value) = character.chars().next() else {
        panic!("character step should supply a char");
    };
    assert_eq!(
        error,
        TemplateError::InvalidCrateNameCharacter {
            character: char_value,
        }
    );
}

#[then("template creation fails with a crate name starting with a non-letter")]
fn then_non_letter_start_error(world: &TemplateWorld) {
    let error = world.error();
    let Some(first_character) = world.crate_name().chars().next() else {
        panic!("crate name must include a starting character");
    };
    assert_eq!(
        error,
        TemplateError::InvalidCrateNameStart {
            character: first_character,
        }
    );
}

#[then("template creation fails due to a trailing separator {separator}")]
fn then_trailing_separator_error(world: &TemplateWorld, separator: StepString) {
    let error = world.error();
    let StepString(separator) = separator;
    let Some(char_value) = separator.chars().next() else {
        panic!("separator step should supply a char");
    };
    assert_eq!(
        error,
        TemplateError::CrateNameTrailingSeparator {
            character: char_value,
        }
    );
}

#[then("template creation fails with an absolute UI directory error pointing to {path}")]
fn then_absolute_directory_error(world: &TemplateWorld, path: StepString) {
    let error = world.error();
    assert_eq!(
        error,
        TemplateError::AbsoluteUiDirectory {
            directory: path.into_inner(),
        }
    );
}

#[then("template creation fails because the UI directory traverses upwards")]
fn then_parent_directory_error(world: &TemplateWorld) {
    let error = world.error();
    assert_eq!(
        error,
        TemplateError::ParentUiDirectory {
            directory: "ui/../secrets".to_string(),
        }
    );
}

#[scenario(path = "tests/features/lint_template.feature", index = 0)]
fn scenario_renders_manifest_and_source(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 1)]
fn scenario_renders_nested_ui_directory(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 2)]
fn scenario_renders_windows_ui_directory(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 3)]
fn scenario_rejects_blank_name(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 4)]
fn scenario_rejects_non_letter_start(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 5)]
fn scenario_rejects_trailing_separator(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 6)]
fn scenario_rejects_absolute_directory(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 7)]
fn scenario_rejects_absolute_windows_directory(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 8)]
fn scenario_rejects_parent_directory(world: TemplateWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/lint_template.feature", index = 9)]
fn scenario_rejects_invalid_character(world: TemplateWorld) {
    let _ = world;
}
