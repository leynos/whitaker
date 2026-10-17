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
    let example_name = ExampleName::new(example);
    let failure = |message: String| example_name.failure(message);
    let package_name = CrateName::try_from(crate_name).map_err(|_| HarnessError::EmptyCrateName)?;
    let metadata = fetch_metadata()?;
    let package = package_for_crate(&metadata, &package_name)?;
    let target = example_target(package, &example_name).ok_or_else(|| {
        failure(format!(
            "{crate_name} declares no example target named `{example}`"
        ))
    })?;

    remove_built_example(&metadata, &example_name).map_err(|error| failure(error.to_string()))?;
    let build_log = build_example_verbosely(package, &example_name).map_err(&failure)?;
    let linking_flags =
        linking_flags_from_build_log(&build_log, &example_name).map_err(&failure)?;
    let directory =
        stage_sources(target.src_path.as_std_path()).map_err(|error| failure(error.to_string()))?;

    Ok(StagedExample {
        directory,
        linking_flags,
    })
}

/// An example target's name, paired with the crate name Cargo compiles it
/// under (the target name with `-` replaced by `_`).
#[derive(Debug)]
struct ExampleName {
    target: String,
    crate_name: String,
}

impl ExampleName {
    fn new(target: &str) -> Self {
        Self {
            target: target.to_owned(),
            crate_name: target.replace('-', "_"),
        }
    }

    fn failure(&self, message: String) -> HarnessError {
        HarnessError::ExampleStagingFailed {
            example: self.target.clone(),
            message,
        }
    }

    /// Whether `path` is build output for this example: the binary itself or
    /// one of Cargo's hashed artefacts beside it.
    fn is_built_artefact(&self, path: &Path) -> bool {
        let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
            return false;
        };
        let binary_name = format!("{}{}", self.crate_name, std::env::consts::EXE_SUFFIX);
        file_name == binary_name || file_name.starts_with(&format!("{}-", self.crate_name))
    }
}

fn example_target<'a>(package: &'a Package, example: &ExampleName) -> Option<&'a Target> {
    package
        .targets
        .iter()
        .find(|target| target.is_example() && target.name == example.target)
}

/// Deletes the example's previous build output.
///
/// Cargo prints a `Running` line only for units it actually compiles, so a
/// fresh example would leave nothing to recover the flags from.
fn remove_built_example(metadata: &Metadata, example: &ExampleName) -> io::Result<()> {
    let examples = metadata.target_directory.join("debug").join("examples");
    let entries = match fs::read_dir(&examples) {
        Ok(entries) => entries,
        // Nothing has been built yet, so there is nothing stale to remove.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let path = entry?.path();
        if example.is_built_artefact(&path) {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn build_example_verbosely(package: &Package, example: &ExampleName) -> Result<String, String> {
    let output = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(package.manifest_path.as_std_path())
        .args(["--example", &example.target, "--verbose"])
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

/// Recovers the linking flags of the single compiler invocation for
/// `example` from a `cargo build --verbose` log.
///
/// Words before the compiler are a wrapper chain (or, at higher verbosity,
/// environment assignments) and are skipped. Terminal styling is stripped
/// first, because CI commonly forces Cargo to style its status words. Like
/// `dylint_testing`, this splits the command on spaces, so paths containing
/// spaces are unsupported.
///
/// # Examples
///
/// ```ignore
/// let log = "     Running `sccache /bin/rustc --crate-name demo --edition=2024 -L dependency=/t`";
/// assert_eq!(
///     linking_flags_from_build_log(log, &ExampleName::new("demo"))?,
///     ["--edition=2024", "-L", "dependency=/t"],
/// );
/// ```
fn linking_flags_from_build_log(log: &str, example: &ExampleName) -> Result<Vec<String>, String> {
    let crate_name = &example.crate_name;
    let mut invocations = log
        .lines()
        .map(LogLine::from_styled)
        .filter_map(|line| line.compiler_invocation())
        .filter(|invocation| invocation.names_crate(example));
    let invocation = invocations
        .next()
        .ok_or_else(|| format!("found no `rustc` invocation for `{crate_name}`"))?;
    if invocations.next().is_some() {
        return Err(format!(
            "found several `rustc` invocations for `{crate_name}`"
        ));
    }
    invocation.linking_flags()
}

/// One line of Cargo output with terminal styling removed.
struct LogLine(String);

impl LogLine {
    /// Removes terminal control sequences (`ESC [ ... final-byte`), which
    /// Cargo wraps around its status words when styling is forced.
    fn from_styled(line: &str) -> Self {
        let mut plain = String::with_capacity(line.len());
        let mut characters = line.chars();
        while let Some(character) = characters.next() {
            if character == '\u{1b}' {
                // Discard the sequence up to and including its alphabetic final byte.
                let _final_byte = characters.by_ref().find(char::is_ascii_alphabetic);
            } else {
                plain.push(character);
            }
        }
        Self(plain)
    }

    /// Parses a line shaped like ``Running `<command>` `` and returns the
    /// arguments after the first word naming `rustc`, if any.
    ///
    /// An assignment such as `RUSTC=/bin/rustc` also has the stem `rustc`, so
    /// words containing `=` are treated as environment, not the compiler.
    fn compiler_invocation(&self) -> Option<CompilerInvocation> {
        let command = self
            .0
            .trim_start()
            .strip_prefix("Running")?
            .trim_start()
            .strip_prefix('`')?
            .strip_suffix('`')?;
        let mut words = command.split(' ');
        words.by_ref().find(|word| {
            !word.contains('=') && Path::new(word).file_stem() == Some(OsStr::new("rustc"))
        })?;
        Some(CompilerInvocation(words.map(str::to_owned).collect()))
    }
}

/// The arguments of one `rustc` invocation, without the compiler path.
struct CompilerInvocation(Vec<String>);

impl CompilerInvocation {
    fn names_crate(&self, example: &ExampleName) -> bool {
        self.0
            .windows(2)
            .any(|pair| matches!(pair, [flag, name] if flag == "--crate-name" && *name == example.crate_name))
    }

    fn linking_flags(&self) -> Result<Vec<String>, String> {
        let mut flags = Vec::new();
        let mut remaining = self.0.iter();
        while let Some(argument) = remaining.next() {
            if argument.starts_with("--edition=") {
                flags.push(argument.clone());
            } else if argument == "--extern" || argument == "-L" {
                let value = remaining
                    .next()
                    .ok_or_else(|| format!("`{argument}` is missing its value"))?;
                flags.extend([argument.clone(), value.trim_matches('\'').to_owned()]);
            }
        }
        Ok(flags)
    }
}

#[cfg(test)]
mod tests;
