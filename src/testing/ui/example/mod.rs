//! Example-target staging for Dylint UI tests.
//!
//! `dylint_testing::ui::Test::example` recovers an example's linking flags by
//! rebuilding it with `cargo build --verbose` and keeping the `Running` line
//! whose *first* word is `rustc`. Any configured `rustc` wrapper becomes that
//! first word instead: `sccache` in CI, or a compile-admission wrapper on a
//! shared host. The wrapper may come from Cargo configuration rather than the
//! environment, so clearing environment variables cannot remove it, and
//! removing it would defeat the caching or admission it exists to provide.
//!
//! This module performs the same recovery, but skips any wrapper chain before
//! the compiler. It then stages the example source and its expectation files
//! in a temporary directory, so callers can hand both to `Test::src_base`,
//! which is exactly what `Test::example` does internally.

use std::{ffi::OsStr, fs, io, path::Path, process::Command};

use cargo_metadata::{Metadata, Package, Target};
use tempfile::TempDir;

use super::{
    HarnessError,
    toolchain::{CrateName, fetch_metadata, package_for_crate},
};

/// Expectation files `compiletest` reads beside a fixture source.
const EXPECTATION_EXTENSIONS: [&str; 3] = ["fixed", "stderr", "stdout"];

/// An example target staged for `dylint_testing::ui::Test::src_base`.
///
/// The staging directory is deleted when this value drops, so keep it alive
/// until the test has run.
#[derive(Debug)]
pub struct StagedExample {
    directory: TempDir,
    linking_flags: Vec<String>,
}

impl StagedExample {
    /// Directory holding the example source and its expectation files.
    #[must_use]
    pub fn directory(&self) -> &Path { self.directory.path() }

    /// The `--edition`, `-L`, and `--extern` flags Cargo used to build the
    /// example, so the staged copy resolves the same dependencies.
    #[must_use]
    pub fn linking_flags(&self) -> &[String] { &self.linking_flags }
}

/// Builds `example` from `crate_name` and stages it for a source-based UI run.
///
/// Use this instead of `dylint_testing::ui::Test::example`, which fails with
/// "Found no `rustc` invocations" whenever a `rustc` wrapper is configured.
///
/// # Errors
///
/// Returns [`HarnessError::ExampleStagingFailed`] when the example cannot be
/// found, built, or copied, or when Cargo's output does not identify exactly
/// one compiler invocation for it.
///
/// # Examples
///
/// ```ignore
/// use dylint_testing::ui::Test;
/// use whitaker::testing::ui::stage_example;
///
/// let staged = stage_example("my_lint", "pass_in_test_harness")?;
/// let mut test = Test::src_base("my_lint", staged.directory());
/// test.rustc_flags(staged.linking_flags()).rustc_flags(["--test"]);
/// test.run();
/// # Ok::<(), whitaker::testing::ui::HarnessError>(())
/// ```
pub fn stage_example(crate_name: &str, example: &str) -> Result<StagedExample, HarnessError> {
    let failure = |message: String| HarnessError::ExampleStagingFailed {
        example: example.to_owned(),
        message,
    };
    let package_name = CrateName::try_from(crate_name).map_err(|_| HarnessError::EmptyCrateName)?;
    let metadata = fetch_metadata()?;
    let package = package_for_crate(&metadata, &package_name)?;
    let target = example_target(package, example).ok_or_else(|| {
        failure(format!(
            "{crate_name} declares no example target named `{example}`"
        ))
    })?;

    remove_built_example(&metadata, example).map_err(|error| failure(error.to_string()))?;
    let build_log = build_example_verbosely(package, example).map_err(&failure)?;
    let linking_flags =
        linking_flags_from_build_log(&build_log, &snake_case(example)).map_err(&failure)?;
    let directory =
        stage_sources(target.src_path.as_std_path()).map_err(|error| failure(error.to_string()))?;

    Ok(StagedExample {
        directory,
        linking_flags,
    })
}

fn example_target<'a>(package: &'a Package, example: &str) -> Option<&'a Target> {
    package
        .targets
        .iter()
        .find(|target| target.is_example() && target.name == example)
}

/// Deletes the example's previous build output.
///
/// Cargo prints a `Running` line only for units it actually compiles, so a
/// fresh example would leave nothing to recover the flags from.
fn remove_built_example(metadata: &Metadata, example: &str) -> io::Result<()> {
    let examples = metadata.target_directory.join("debug").join("examples");
    let entries = match fs::read_dir(&examples) {
        Ok(entries) => entries,
        // Nothing has been built yet, so there is nothing stale to remove.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let stem = snake_case(example);
    for entry in entries {
        let path = entry?.path();
        if is_built_example(&path, &stem) {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn is_built_example(path: &Path, stem: &str) -> bool {
    let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    let binary_name = format!("{stem}{}", std::env::consts::EXE_SUFFIX);
    file_name == binary_name || file_name.starts_with(&format!("{stem}-"))
}

fn build_example_verbosely(package: &Package, example: &str) -> Result<String, String> {
    let output = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(package.manifest_path.as_std_path())
        .args(["--example", example, "--verbose", "--color", "never"])
        .output()
        .map_err(|error| format!("failed to run cargo: {error}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        Ok(stderr)
    } else {
        Err(format!("cargo build failed:\n{stderr}"))
    }
}

fn stage_sources(source: &Path) -> io::Result<TempDir> {
    let directory = tempfile::tempdir()?;
    let file_name = source
        .file_name()
        .ok_or_else(|| io::Error::other(format!("{} has no file name", source.display())))?;
    let staged = directory.path().join(file_name);
    fs::copy(source, &staged)?;
    for extension in EXPECTATION_EXTENSIONS {
        copy_if_present(
            &source.with_extension(extension),
            &staged.with_extension(extension),
        )?;
    }
    Ok(directory)
}

fn copy_if_present(from: &Path, to: &Path) -> io::Result<()> {
    match fs::copy(from, to) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Cargo names an example's crate after the target, with `-` replaced.
fn snake_case(name: &str) -> String { name.replace('-', "_") }

/// Recovers the linking flags of the single compiler invocation for
/// `crate_name` from a `cargo build --verbose` log.
///
/// Words before the compiler are a wrapper chain (or, at higher verbosity,
/// environment assignments) and are skipped. Like `dylint_testing`, this
/// splits the command on spaces, so paths containing spaces are unsupported.
///
/// # Examples
///
/// ```ignore
/// let log = "     Running `sccache /bin/rustc --crate-name demo --edition=2024 -L dependency=/t`";
/// assert_eq!(
///     linking_flags_from_build_log(log, "demo")?,
///     ["--edition=2024", "-L", "dependency=/t"],
/// );
/// ```
fn linking_flags_from_build_log(log: &str, crate_name: &str) -> Result<Vec<String>, String> {
    let mut invocations = log
        .lines()
        .filter_map(running_command)
        .filter_map(compiler_arguments)
        .filter(|arguments| names_crate(arguments, crate_name));
    let arguments = invocations
        .next()
        .ok_or_else(|| format!("found no `rustc` invocation for `{crate_name}`"))?;
    if invocations.next().is_some() {
        return Err(format!(
            "found several `rustc` invocations for `{crate_name}`"
        ));
    }
    linking_flags(&arguments)
}

/// Extracts the command from a line shaped like ``Running `<command>` ``.
fn running_command(line: &str) -> Option<&str> {
    line.trim_start()
        .strip_prefix("Running")?
        .trim_start()
        .strip_prefix('`')?
        .strip_suffix('`')
}

/// Returns the arguments after the first word naming `rustc`, if any.
fn compiler_arguments(command: &str) -> Option<Vec<&str>> {
    let mut words = command.split(' ');
    words.by_ref().find(|word| is_rustc(word))?;
    Some(words.collect())
}

/// An assignment such as `RUSTC=/bin/rustc` also has the stem `rustc`, so
/// words containing `=` are environment, not the compiler.
fn is_rustc(word: &str) -> bool {
    !word.contains('=') && Path::new(word).file_stem() == Some(OsStr::new("rustc"))
}

fn names_crate(arguments: &[&str], crate_name: &str) -> bool {
    arguments
        .windows(2)
        .any(|pair| pair == ["--crate-name", crate_name])
}

fn linking_flags(arguments: &[&str]) -> Result<Vec<String>, String> {
    let mut flags = Vec::new();
    let mut remaining = arguments.iter();
    while let Some(&argument) = remaining.next() {
        if argument.starts_with("--edition=") {
            flags.push(argument.to_owned());
        } else if argument == "--extern" || argument == "-L" {
            let value = remaining
                .next()
                .ok_or_else(|| format!("`{argument}` is missing its value"))?;
            flags.extend([argument.to_owned(), value.trim_matches('\'').to_owned()]);
        }
    }
    Ok(flags)
}

#[cfg(test)]
mod tests;
