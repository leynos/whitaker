//! Example-target UI tests: recover each example's compiler flags from Cargo.
//!
//! Whitaker patch: upstream `dylint_testing` 6.3.0 keeps this in `lib.rs`;
//! Whitaker keeps it in its own module, as its 6.0.1 patch did, so each file
//! stays within the repository's 400-line limit. The functions are upstream's
//! apart from the changes marked "Whitaker patch".

use crate::{copy_with_extension, run_tests, snake_case, ui};
use anyhow::{Context, Result, anyhow, ensure};
use cargo_metadata::{Metadata, Package, Target, TargetKind};
use dylint_internal::{CommandExt, env, rustup::is_rustc};
use once_cell::sync::OnceCell;
use std::{fs::copy, io::BufRead, path::Path};

static LINKING_FLAGS: OnceCell<Vec<String>> = OnceCell::new();

pub(crate) fn example_target(package: &Package, example: &str) -> Result<Target> {
    package
        .targets
        .iter()
        .find(|target| target.kind == [TargetKind::Example] && target.name == example)
        .cloned()
        .ok_or_else(|| anyhow!("Could not find example `{example}`"))
}

/// List the package's example targets.
///
/// Whitaker patch: returns the list directly; upstream wraps it in a `Result`
/// that cannot fail and silences `clippy::unnecessary_wraps`.
pub(crate) fn example_targets(package: &Package) -> Vec<Target> {
    package
        .targets
        .iter()
        .filter(|target| target.kind == [TargetKind::Example])
        .cloned()
        .collect()
}

/// The inputs for one example UI test.
///
/// Whitaker patch: upstream passes these as five arguments, which Whitaker's
/// argument limit rejects; grouping them is the only change to the function.
pub(crate) struct ExampleTestContext<'a> {
    pub(crate) driver: &'a Path,
    pub(crate) metadata: &'a Metadata,
    pub(crate) package: &'a Package,
    pub(crate) target: &'a Target,
    pub(crate) config: &'a ui::Config,
}

pub(crate) fn run_example_test(context: &ExampleTestContext<'_>) -> Result<()> {
    let ExampleTestContext {
        driver,
        metadata,
        package,
        target,
        config,
    } = *context;
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

    let mut config = config.clone();
    config.rustc_flags.extend(linking_flags.iter().cloned());

    run_tests(driver, src_base, &config);

    Ok(())
}

fn linking_flags(
    metadata: &Metadata,
    package: &Package,
    target: &Target,
) -> Result<&'static [String]> {
    LINKING_FLAGS
        .get_or_try_init(|| select_ui_test_flags(rustc_flags(metadata, package, target)?))
        .map(Vec::as_slice)
}

/// Retain the Cargo flags needed to compile UI tests without forwarding linker paths.
///
/// Whitaker patch: `compiletest` flattens its rustc flags into a
/// space-delimited string. Native linker paths are not needed for
/// metadata-only UI tests and can contain spaces on Windows (`C:\Program
/// Files\...`), where flattening would split them into extra input files.
///
/// # Errors
///
/// Returns an error if a value-taking compiler flag is missing its value.
fn select_ui_test_flags(rustc_flags: Vec<String>) -> Result<Vec<String>> {
    let mut ui_test_flags = Vec::new();
    let mut iter = rustc_flags.into_iter();
    while let Some(flag) = iter.next() {
        if flag.starts_with("--edition=") {
            ui_test_flags.push(flag);
        } else if flag == "--extern" || flag == "-L" {
            let arg = next(&flag, &mut iter)?;
            let arg = arg.trim_matches('\'').to_owned();
            if flag != "-L" || !arg.starts_with("native=") {
                ui_test_flags.extend([flag, arg]);
            }
        }
    }
    Ok(ui_test_flags)
}

// smoelius: We need to recover the `rustc` flags used to build a target. I can see four options:
//
// * Use `cargo build --build-plan`
//   - Pros: Easily parsable JSON output
//   - Cons: Unstable and likely to be removed: https://github.com/rust-lang/cargo/issues/7614
// * Parse the output of `cargo build --verbose`
//   - Pros: ?
//   - Cons: Not as easily parsable, requires synchronization (see below)
// * Use a custom executor like Siderophile does: https://github.com/trailofbits/siderophile/blob/26c067306f6c2f66d9530dacef6b17dbf59cdf8c/src/trawl_source/mod.rs#L399
//   - Pros: Ground truth
//   - Cons: Seems a bit of a heavy lift (Note: I think Siderophile's approach was inspired by
//     `cargo-geiger`.)
// * Set `RUSTC_WORKSPACE_WRAPPER` to something that logs `rustc` invocations
//   - Pros: Ground truth
//   - Cons: Requires a separate executable/script, portability could be an issue
//
// I am going with the second option for now, because it seems to be the least of all evils. This
// decision may need to be revisited.

/// Return the command inside one of Cargo's ``Running `...` `` lines.
///
/// Whitaker patch: matches what upstream's `^\s*Running\s*`(.*)`$` regex
/// matched, without a fallible regex initialization.
fn running_command(line: &str) -> Option<&str> {
    line.trim_start()
        .strip_prefix("Running")?
        .trim_start()
        .strip_prefix('`')?
        .strip_suffix('`')
}

fn rustc_flags(_metadata: &Metadata, package: &Package, target: &Target) -> Result<Vec<String>> {
    // Previously, this function removed the example executable from `target/debug/examples` before
    // running `cargo build --verbose`. With Cargo's legacy layout, that forced the example to be
    // recompiled and exposed its rustc invocation in Cargo's verbose output. With the new
    // build-directory layout, Cargo can restore the removed executable from its cached build
    // artifact without invoking rustc, making removal ineffective as a way to force recompilation.
    // So, use `cargo rustc` with a process-specific `-C metadata` value instead. The changed
    // metadata forces rustc to run for the selected example without removing artifacts or relying
    // on Cargo's internal layout.
    // Whitaker patch: a process ID alone can repeat once a later test process
    // reuses it, and Cargo then finds the example fresh, runs no `rustc`, and
    // the parser finds no invocation. The clock makes the value unique.
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let rustc_metadata = format!("metadata=dylint_testing_{}_{started}", std::process::id());
    let output = {
        // smoelius: Because of lazy initialization, `cargo rustc` is run only once. Seeing
        // "Building example `target`" for one example but not for others is confusing. So instead
        // say "Building `package` examples".
        dylint_internal::cargo::rustc(&format!("`{}` examples", package.name))
            .build()
            .env_remove(env::CARGO_TERM_COLOR)
            .args([
                "--manifest-path",
                package.manifest_path.as_ref(),
                "--example",
                &target.name,
                "--verbose",
                "--",
                "-C",
                &rustc_metadata,
            ])
            .logged_output(true)?
    };

    let matches = output
        .stderr
        .lines()
        .map(|line| {
            let line =
                line.with_context(|| format!("Could not read from `{}`", package.manifest_path))?;
            // `rustc_invocation` matches `RE` itself, so it takes the whole line.
            Ok(rustc_invocation(&line, &snake_case(&target.name)))
        })
        .collect::<Result<Vec<Option<Vec<_>>>>>()?;

    let mut matches = matches.into_iter().flatten().collect::<Vec<Vec<_>>>();
    ensure!(
        matches.len() <= 1,
        "Found multiple `rustc` invocations for `{}`",
        target.name
    );
    matches
        .pop()
        .ok_or_else(|| anyhow!("Found no `rustc` invocations for `{}`", target.name))
}

/// Extract the requested compiler arguments from Cargo's rendered command line.
///
/// Cargo may prefix the compiler with one or more configured wrappers, such as
/// `sccache` or a cache-coordinating shim. Splitting the raw line on spaces
/// attributes the wrapper's own arguments to `rustc`; `args.first()` then fails
/// to recognize the compiler, no invocation is found, and the UI test aborts.
/// Parse the displayed quoting instead, then return only the argument tail after
/// the `rustc` preceding the crate name. For example, a wrapped
/// `wrapper rustc --crate-name demo` command yields `--crate-name demo` without
/// the wrapper arguments.
///
/// The last `rustc` before the crate name is chosen rather than the first, so a
/// wrapper that is itself named `rustc` cannot shadow the compiler it wraps.
fn rustc_invocation(line: &str, target_crate: &str) -> Option<Vec<String>> {
    let command = running_command(line)?;
    let arguments = shlex::split(command)?;
    let crate_name_index = arguments
        .windows(2)
        .position(|window| window[0] == "--crate-name" && window[1] == target_crate)?;
    let rustc_index = arguments[..crate_name_index].iter().rposition(is_rustc)?;
    Some(
        arguments
            .into_iter()
            .skip(rustc_index + 1)
            .collect::<Vec<_>>(),
    )
}

fn next<I, T>(flag: &str, iter: &mut I) -> Result<T>
where
    I: Iterator<Item = T>,
{
    iter.next()
        .ok_or_else(|| anyhow!("Missing argument for `{flag}`"))
}

#[cfg(test)]
mod tests {
    use super::{running_command, rustc_invocation, select_ui_test_flags};
    use rstest::rstest;

    /// Extract the rustc arguments after unwrapped and wrapped compiler
    /// commands.
    #[rstest]
    #[case::unwrapped(
        r"Running `/toolchain/bin/rustc --crate-name target --edition=2024`",
        &["--crate-name", "target", "--edition=2024"],
    )]
    #[case::single_wrapper(
        r"Running `/cache/sccache /toolchain/bin/rustc --crate-name target --edition=2024`",
        &["--crate-name", "target", "--edition=2024"],
    )]
    #[case::nested_wrappers(
        r"Running `/cache/sccache /cache/build-limits-rustc /toolchain/bin/rustc --crate-name target --edition=2024`",
        &["--crate-name", "target", "--edition=2024"],
    )]
    #[case::wrapper_named_rustc(
        r"Running `/cache/rustc /toolchain/bin/rustc --crate-name target --edition=2024`",
        &["--crate-name", "target", "--edition=2024"],
    )]
    #[case::quoted_paths(
        r#"Running `"/tool chain/sccache" "/tool chain/bin/rustc" --crate-name target --extern "dependency=/build tree/libdependency.rlib"`"#,
        &[
            "--crate-name",
            "target",
            "--extern",
            "dependency=/build tree/libdependency.rlib",
        ],
    )]
    fn parses_rustc_arguments_after_wrappers(#[case] line: &str, #[case] expected: &[&str]) {
        let parsed = rustc_invocation(line, "target");
        let expected = expected
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>();

        assert_eq!(parsed, Some(expected));
    }

    /// Ignore unrelated Cargo output and commands for other compiler targets.
    #[rstest]
    #[case::not_a_cargo_command("Compiling target v0.1.0")]
    #[case::not_a_compiler(r"Running `/toolchain/bin/cargo --crate-name target`")]
    #[case::wrong_crate(r"Running `/toolchain/bin/rustc --crate-name another`")]
    #[case::malformed_quotes(r#"Running `wrapper "/toolchain/bin/rustc --crate-name target`"#)]
    // Regression: the 6.3.0 rebase passed the regex's inner capture here, so
    // every example test found no invocation. The helper takes the whole line.
    #[case::already_captured_command(r"/toolchain/bin/rustc --crate-name target")]
    fn ignores_commands_without_the_requested_rustc_invocation(#[case] line: &str) {
        assert_eq!(rustc_invocation(line, "target"), None);
    }

    /// Preserve backslashes while splitting quoted Windows executable paths.
    #[test]
    fn splits_windows_paths_without_losing_backslashes() {
        assert_eq!(
            shlex::split(
                r#""C:\Program Files\sccache.exe" "C:\toolchain\bin\rustc.exe" --crate-name target"#
            ),
            Some(vec![
                r"C:\Program Files\sccache.exe".to_owned(),
                r"C:\toolchain\bin\rustc.exe".to_owned(),
                "--crate-name".to_owned(),
                "target".to_owned(),
            ])
        );
    }

    /// Regression: the 6.3.0 rebase dropped this filter, and Windows example
    /// tests failed with "multiple input filenames provided" because a
    /// `native=C:\Program Files\...` path was split at its space.
    #[test]
    fn selects_ui_flags_without_native_linker_paths() {
        let flags = select_ui_test_flags(vec![
            "--edition=2024".to_owned(),
            "--extern".to_owned(),
            "dependency=target/debug/deps/libdependency.rlib".to_owned(),
            "-L".to_owned(),
            "dependency=target/debug/deps".to_owned(),
            "-L".to_owned(),
            r"native=C:\Program Files\Microsoft Visual Studio\VC\Tools\MSVC\lib\x64".to_owned(),
        ])
        .expect("complete Cargo flags should be selected");

        assert_eq!(
            flags,
            [
                "--edition=2024",
                "--extern",
                "dependency=target/debug/deps/libdependency.rlib",
                "-L",
                "dependency=target/debug/deps",
            ]
        );
    }

    /// The parser accepts exactly the lines upstream's regex accepted.
    #[rstest]
    #[case::plain("Running `cargo build`", Some("cargo build"))]
    #[case::indented("     Running `rustc --crate-name x`", Some("rustc --crate-name x"))]
    #[case::no_space("Running`rustc`", Some("rustc"))]
    #[case::inner_backticks("Running `a `b` c`", Some("a `b` c"))]
    #[case::unterminated("Running `rustc", None)]
    #[case::trailing_text("Running `rustc` now", None)]
    #[case::other_verb("Compiling `rustc`", None)]
    fn parses_cargo_running_lines(#[case] line: &str, #[case] expected: Option<&str>) {
        assert_eq!(running_command(line), expected);
    }
}
