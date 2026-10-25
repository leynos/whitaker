//! Resolves example targets and extracts the compiler flags needed to test them.

use std::{
    env::consts,
    fs::{read_dir, remove_file},
    io::BufRead,
    sync::LazyLock,
};

use anyhow::{Context, Result, anyhow, ensure};
use cargo_metadata::{Metadata, Package, Target, TargetKind};
use dylint_internal::{CommandExt, env, rustup::is_rustc};
use regex::Regex;

pub(super) fn example_target(package: &Package, example: &str) -> Result<Target> {
    package
        .targets
        .iter()
        .find(|target| target.kind == [TargetKind::Example] && target.name == example)
        .cloned()
        .ok_or_else(|| anyhow!("Could not find example `{example}`"))
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

pub(super) fn rustc_flags(
    metadata: &Metadata,
    package: &Package,
    target: &Target,
) -> Result<Vec<String>> {
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
            Ok((*RE).captures(&line).and_then(|captures| {
                let args = captures[1]
                    .split(' ')
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                let rustc_args = compiler_arguments(&args)?;
                if rustc_args
                    .windows(2)
                    .any(|window| window == ["--crate-name", &snake_case(&target.name)])
                {
                    Some(rustc_args.to_vec())
                } else {
                    None
                }
            }))
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

/// Returns the compiler arguments after any configured rustc wrappers.
fn compiler_arguments(args: &[String]) -> Option<&[String]> {
    args.iter()
        .position(|argument| is_rustc(argument))
        .map(|index| &args[index..])
}

#[cfg(test)]
mod compiler_arguments_tests {
    use super::compiler_arguments;

    #[test]
    fn finds_rustc_after_managed_wrapper() {
        let args = vec![
            "/home/user/.local/bin/build-limits-rustc".to_owned(),
            "/toolchain/bin/rustc".to_owned(),
            "--crate-name".to_owned(),
            "sample".to_owned(),
        ];

        assert_eq!(compiler_arguments(&args), Some(&args[1..]));
    }

    #[test]
    fn retains_bare_rustc_invocation() {
        let args = vec![
            "/toolchain/bin/rustc".to_owned(),
            "--crate-name".to_owned(),
            "sample".to_owned(),
        ];

        assert_eq!(compiler_arguments(&args), Some(&args[..]));
    }
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

pub(super) fn next<I, T>(flag: &str, iter: &mut I) -> Result<T>
where
    I: Iterator<Item = T>,
{
    iter.next()
        .ok_or_else(|| anyhow!("Missing argument for `{flag}`"))
}

fn snake_case(name: &str) -> String { name.replace('-', "_") }
