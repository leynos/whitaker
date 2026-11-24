//! UI-test builder: `ui::Test` configures and runs one Dylint UI test.
//!
//! A test targets either a source directory (`Test::src_base`), one example
//! (`Test::example`), or every example of the current package
//! (`Test::examples`). `Test::run` initializes the driver with `initialize`,
//! then dispatches on the target.
//!
//! Source-directory tests call the crate-level `run_tests` helper directly.
//! Example tests resolve the package's example targets with `example_target`
//! or `example_targets`, bundle the driver, metadata, package, target, and
//! configuration into an `ExampleTestContext`, and pass it to
//! `run_example_test`. The shared `Config` carries the extra compiler flags
//! and `dylint.toml` contents set on the builder.

use std::{
    env::current_dir,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use crate::{
    ExampleTestContext,
    example_target,
    example_targets,
    initialize,
    run_example_test,
    run_tests,
};

enum Target {
    SrcBase(PathBuf),
    Example(String),
    Examples,
}

#[derive(Clone, Default)]
pub(super) struct Config {
    pub(super) rustc_flags: Vec<String>,
    pub(super) dylint_toml: Option<String>,
    /// Whitaker patch: variables the compiler and the lint driver should see
    /// for this run, where `None` removes the variable.
    pub(super) env: Vec<(String, Option<OsString>)>,
}

/// Test builder
pub struct Test {
    name: String,
    target: Target,
    config: Config,
}

impl Test {
    /// Test a library on all source files in a directory (similar to [`ui_test`]).
    ///
    /// [`ui_test`]: crate::ui_test
    #[must_use]
    pub fn src_base(name: &str, src_base: impl AsRef<Path>) -> Self {
        Self::new(name, Target::SrcBase(src_base.as_ref().to_owned()))
    }

    /// Test a library on one example target (similar to [`ui_test_example`]).
    ///
    /// [`ui_test_example`]: crate::ui_test_example
    #[must_use]
    pub fn example(name: &str, example: &str) -> Self {
        Self::new(name, Target::Example(example.to_owned()))
    }

    /// Test a library on all example targets (similar to [`ui_test_examples`]).
    ///
    /// [`ui_test_examples`]: crate::ui_test_examples
    #[must_use]
    pub fn examples(name: &str) -> Self { Self::new(name, Target::Examples) }

    /// Pass flags to the compiler when running the test.
    pub fn rustc_flags(
        &mut self,
        rustc_flags: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> &mut Self {
        self.config
            .rustc_flags
            .extend(rustc_flags.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    /// Set the `dylint.toml` file's contents (for testing configurable libraries).
    pub fn dylint_toml(&mut self, dylint_toml: impl AsRef<str>) -> &mut Self {
        self.config.dylint_toml = Some(dylint_toml.as_ref().to_owned());
        self
    }

    /// Whitaker patch: run the test with `key` set to `value`.
    ///
    /// `compiletest` has no environment setting of its own, so the variable
    /// is applied to the process for the duration of the run, under the lock
    /// that already serializes runs, and the previous value is restored
    /// afterwards. This keeps the change inside one place instead of every
    /// caller mutating the process around the run. A later setting of the same
    /// key replaces an earlier one.
    pub fn env(&mut self, key: impl AsRef<str>, value: impl AsRef<OsStr>) -> &mut Self {
        self.set_env(key.as_ref(), Some(value.as_ref().to_owned()))
    }

    /// Whitaker patch: run the test with `key` removed from the environment.
    ///
    /// See [`Test::env`] for how the change is scoped.
    pub fn env_removed(&mut self, key: impl AsRef<str>) -> &mut Self {
        self.set_env(key.as_ref(), None)
    }

    fn set_env(&mut self, key: &str, value: Option<OsString>) -> &mut Self {
        self.config.env.retain(|(existing, _)| existing != key);
        self.config.env.push((key.to_owned(), value));
        self
    }

    /// Run the test.
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn run(&mut self) { self.run_immutable(); }

    fn new(name: &str, target: Target) -> Self {
        Self {
            name: name.to_owned(),
            target,
            config: Config::default(),
        }
    }

    fn run_immutable(&self) {
        let driver = initialize(&self.name).as_ref().unwrap();

        match &self.target {
            Target::SrcBase(src_base) => {
                run_tests(driver, src_base, &self.config).unwrap();
            }
            Target::Example(example) => {
                let metadata = dylint_internal::cargo::current_metadata().unwrap();
                let current_dir = current_dir().unwrap();
                let package =
                    dylint_internal::cargo::package_with_root(&metadata, &current_dir).unwrap();
                let target = example_target(&package, example).unwrap();

                run_example_test(&ExampleTestContext {
                    driver,
                    metadata: &metadata,
                    package: &package,
                    target: &target,
                    config: &self.config,
                })
                .unwrap();
            }
            Target::Examples => {
                let metadata = dylint_internal::cargo::current_metadata().unwrap();
                let current_dir = current_dir().unwrap();
                let package =
                    dylint_internal::cargo::package_with_root(&metadata, &current_dir).unwrap();
                let targets = example_targets(&package);

                for target in targets {
                    run_example_test(&ExampleTestContext {
                        driver,
                        metadata: &metadata,
                        package: &package,
                        target: &target,
                        config: &self.config,
                    })
                    .unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // smoelius: Verify that `rustc_flags` compiles when used as intended.
    #[allow(dead_code)]
    fn rustc_flags() { let _ = Test::src_base("name", PathBuf::new()).rustc_flags(["--test"]); }

    #[test]
    fn a_later_setting_of_a_key_replaces_an_earlier_one() {
        let mut test = Test::src_base("name", PathBuf::new());
        test.env("KEY", "first")
            .env("OTHER", "kept")
            .env_removed("KEY");

        assert_eq!(
            test.config.env,
            vec![
                ("OTHER".to_owned(), Some(OsString::from("kept"))),
                ("KEY".to_owned(), None),
            ]
        );
    }
}
