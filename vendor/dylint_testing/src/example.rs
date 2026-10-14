//! Example-target discovery and compiler-flag extraction for Dylint UI tests.

use anyhow::{Context, Result, anyhow, ensure};
use cargo_metadata::{Metadata, Package, Target, TargetKind};
use dylint_internal::{CommandExt, env, rustup::is_rustc};
use regex::Regex;
use std::{
    env::consts,
    fs::{copy, read_dir, remove_file},
    io::BufRead,
    path::Path,
    sync::LazyLock,
};

use crate::{run_tests, snake_case, ui};

static UI_TEST_FLAGS: once_cell::sync::OnceCell<Vec<String>> = once_cell::sync::OnceCell::new();

/// Find the example target named `example` in `package`.
pub fn example_target(package: &Package, example: &str) -> Result<Target> {
    package
        .targets
        .iter()
        .find(|target| target.kind == [TargetKind::Example] && target.name == example)
        .cloned()
        .ok_or_else(|| anyhow!("Could not find example `{example}`"))
}

/// Return every example target in `package`.
#[allow(clippy::unnecessary_wraps)]
pub fn example_targets(package: &Package) -> Result<Vec<Target>> {
    Ok(package
        .targets
        .iter()
        .filter(|target| target.kind == [TargetKind::Example])
        .cloned()
        .collect())
}

/// Context for running a UI test against one Cargo example target.
#[derive(Clone, Copy)]
pub struct ExampleTestContext<'a> {
    /// Dylint driver used to execute compiler tests.
    pub driver: &'a Path,
    /// Cargo metadata for the workspace under test.
    pub metadata: &'a Metadata,
    /// Package that owns the selected example target.
    pub package: &'a Package,
    /// Example target whose compiler flags are reused.
    pub target: &'a Target,
    /// Configuration applied to the UI test.
    pub config: &'a ui::Config,
}

/// Run a UI test using compiler flags from the selected example target.
pub fn run_example_test(context: ExampleTestContext<'_>) -> Result<()> {
    let ExampleTestContext {
        driver,
        metadata,
        package,
        target,
        config,
    } = context;
    let ui_test_flags = ui_test_flags(metadata, package, target)?;
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
    config.rustc_flags.extend(ui_test_flags.iter().cloned());

    run_tests(driver, src_base, &config);

    Ok(())
}

fn ui_test_flags(
    metadata: &Metadata,
    package: &Package,
    target: &Target,
) -> Result<&'static [String]> {
    UI_TEST_FLAGS
        .get_or_try_init(|| {
            let rustc_flags = rustc_flags(metadata, package, target)?;
            select_ui_test_flags(rustc_flags)
        })
        .map(Vec::as_slice)
}

/// Retain the Cargo flags needed to compile UI tests without forwarding linker paths.
///
/// `compiletest` flattens its rustc flags into a space-delimited string. Native
/// linker paths are not needed for metadata-only UI tests and can contain spaces
/// on Windows, where flattening would split them into multiple arguments.
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
            let argument = next(&flag, &mut iter)?;
            if flag != "-L" || !argument.starts_with("native=") {
                ui_test_flags.extend([flag, argument]);
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

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*Running\s*`(.*)`$").unwrap());

fn rustc_flags(metadata: &Metadata, package: &Package, target: &Target) -> Result<Vec<String>> {
    // smoelius: The following comments are old and retained for posterity. The linking flags are
    // now initialized using a `OnceCell`, which makes the mutex unnecessary.
    //   smoelius: Force rebuilding of the example by removing it. This is kind of messy. The
    //   example is a shared resource that may be needed by multiple tests. For now, I lock a mutex
    //   while the example is removed and put back.
    //   smoelius: Should we use a temporary target directory here?
    let output = {
        remove_example(metadata, package, target)?;

        // smoelius: Because of lazy initialization, `cargo build` is run only once. Seeing
        // "Building example `target`" for one example but not for others is confusing. So instead
        // say "Building `package` examples".
        dylint_internal::cargo::build(&format!("`{}` examples", package.name))
            .build()
            .env_remove(env::CARGO_TERM_COLOR)
            .args([
                "--manifest-path",
                package.manifest_path.as_ref(),
                "--example",
                &target.name,
                "--verbose",
            ])
            .logged_output(true)?
    };

    let matches = output
        .stderr
        .lines()
        .map(|line| {
            let line =
                line.with_context(|| format!("Could not read from `{}`", package.manifest_path))?;
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
/// Cargo may prefix the compiler with one or more configured wrappers. Parse
/// the displayed quoting, then return only the argument tail after the actual
/// `rustc` executable. For example, a wrapped `rustc --crate-name demo` command
/// yields `--crate-name demo`, without the wrapper arguments.
fn rustc_invocation(line: &str, target_crate: &str) -> Option<Vec<String>> {
    let command = RE.captures(line)?.get(1)?.as_str();
    let arguments = shlex::split(command)?;
    let crate_name_index = arguments
        .windows(2)
        .position(|window| window[0] == "--crate-name" && window[1] == target_crate)?;
    let rustc_index = arguments[..crate_name_index].iter().rposition(is_rustc)?;
    let rustc_arguments = arguments
        .into_iter()
        .skip(rustc_index + 1)
        .collect::<Vec<_>>();

    Some(rustc_arguments)
}

fn remove_example(metadata: &Metadata, _package: &Package, target: &Target) -> Result<()> {
    let examples = metadata.target_directory.join("debug/examples");
    for entry in
        read_dir(&examples).with_context(|| format!("`read_dir` failed for `{examples}`"))?
    {
        let entry = entry.with_context(|| format!("`read_dir` failed for `{examples}`"))?;
        let path = entry.path();

        let file_name = entry.file_name();
        let s = file_name.to_string_lossy();
        let target_name = snake_case(&target.name);
        if s == target_name.clone() + consts::EXE_SUFFIX
            || s.starts_with(&(target_name.clone() + "-"))
        {
            remove_file(&path).with_context(|| {
                format!("`remove_file` failed for `{}`", path.to_string_lossy())
            })?;
        }
    }

    Ok(())
}

fn next<I, T>(flag: &str, iter: &mut I) -> Result<T>
where
    I: Iterator<Item = T>,
{
    iter.next()
        .ok_or_else(|| anyhow!("Missing argument for `{flag}`"))
}

fn copy_with_extension<P: AsRef<Path>, Q: AsRef<Path>>(
    from: P,
    to: Q,
    extension: &str,
) -> Result<u64> {
    let from = from.as_ref().with_extension(extension);
    let to = to.as_ref().with_extension(extension);
    copy(from, to).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{rustc_invocation, select_ui_test_flags};
    use rstest::rstest;

    /// Extract the rustc arguments after unwrapped and wrapped compiler commands.
    #[rstest]
    #[case::unwrapped(
        r"Running `/toolchain/bin/rustc --crate-name target --edition=2024`",
        &["--crate-name", "target", "--edition=2024"],
    )]
    #[case::single_wrapper(
        r"Running `/cache/build-limits-rustc /toolchain/bin/rustc --crate-name target --edition=2024`",
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
        r#"Running `"/tool chain/build-limits-rustc" "/tool chain/bin/rustc" --crate-name target --extern "dependency=/build tree/libdependency.rlib"`"#,
        &["--crate-name", "target", "--extern", "dependency=/build tree/libdependency.rlib"],
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
    fn ignores_commands_without_the_requested_rustc_invocation(#[case] line: &str) {
        assert_eq!(rustc_invocation(line, "target"), None);
    }

    /// Preserve backslashes while splitting quoted Windows executable paths.
    #[test]
    fn splits_windows_paths_without_losing_backslashes() {
        assert_eq!(
            shlex::split(
                r#""C:\Program Files\wrapper.exe" "C:\toolchain\bin\rustc.exe" --crate-name target"#
            ),
            Some(vec![
                r"C:\Program Files\wrapper.exe".to_owned(),
                r"C:\toolchain\bin\rustc.exe".to_owned(),
                "--crate-name".to_owned(),
                "target".to_owned(),
            ])
        );
    }

    /// Preserve dependency flags while omitting a spaced Windows linker path.
    #[test]
    fn omits_native_search_paths_from_metadata_only_ui_flags() {
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
}
