use std::{
    env::current_dir,
    fs::copy,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use cargo_metadata::{Metadata, Package, Target as CargoTarget};

use crate::{
    copy_with_extension,
    example_target,
    example_targets,
    initialize,
    linking_flags,
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
                run_tests(driver, src_base, &self.config);
            }
            Target::Example(example) => {
                let metadata = dylint_internal::cargo::current_metadata().unwrap();
                let current_dir = current_dir().unwrap();
                let package =
                    dylint_internal::cargo::package_with_root(&metadata, &current_dir).unwrap();
                let target = example_target(&package, example).unwrap();

                self.run_example_test(&metadata, &package, &target).unwrap();
            }
            Target::Examples => {
                let metadata = dylint_internal::cargo::current_metadata().unwrap();
                let current_dir = current_dir().unwrap();
                let package =
                    dylint_internal::cargo::package_with_root(&metadata, &current_dir).unwrap();
                let targets = example_targets(&package).unwrap();

                for target in targets {
                    self.run_example_test(&metadata, &package, &target).unwrap();
                }
            }
        }
    }

    fn run_example_test(
        &self,
        metadata: &Metadata,
        package: &Package,
        target: &CargoTarget,
    ) -> Result<()> {
        let driver = initialize(&self.name).as_ref().unwrap();
        let linking_flags = linking_flags(metadata, package, target)?;
        let file_name = target
            .src_path
            .file_name()
            .ok_or_else(|| anyhow!("Could not get file name"))?;

        let tempdir = tempfile::tempdir().with_context(|| "`tempdir` failed")?;
        let src_base = tempdir.path();
        let to = src_base.join(file_name);

        copy(&target.src_path, &to).with_context(|| {
            format!(
                "Could not copy `{}` to `{}`",
                target.src_path,
                to.to_string_lossy()
            )
        })?;
        for extension in ["fixed", "stderr", "stdout"] {
            copy_with_extension(&target.src_path, &to, extension)
                .map(|_| ())
                .unwrap_or_default();
        }

        let mut config = self.config.clone();
        config.rustc_flags.extend(linking_flags.iter().cloned());

        run_tests(driver, src_base, &config);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // smoelius: Verify that `rustc_flags` compiles when used as intended.
    #[allow(dead_code)]
    fn rustc_flags() { let _ = Test::src_base("name", PathBuf::new()).rustc_flags(["--test"]); }
}
