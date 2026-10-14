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

static LINKING_FLAGS: once_cell::sync::OnceCell<Vec<String>> = once_cell::sync::OnceCell::new();

pub(crate) fn example_target(package: &Package, example: &str) -> Result<Target> {
    package
        .targets
        .iter()
        .find(|target| target.kind == [TargetKind::Example] && target.name == example)
        .cloned()
        .ok_or_else(|| anyhow!("Could not find example `{example}`"))
}

#[allow(clippy::unnecessary_wraps)]
pub(crate) fn example_targets(package: &Package) -> Result<Vec<Target>> {
    Ok(package
        .targets
        .iter()
        .filter(|target| target.kind == [TargetKind::Example])
        .cloned()
        .collect())
}

pub(crate) fn run_example_test(
    driver: &Path,
    metadata: &Metadata,
    package: &Package,
    target: &Target,
    config: &ui::Config,
) -> Result<()> {
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
        .get_or_try_init(|| {
            let rustc_flags = rustc_flags(metadata, package, target)?;

            let mut linking_flags = Vec::new();

            let mut iter = rustc_flags.into_iter();
            while let Some(flag) = iter.next() {
                if flag.starts_with("--edition=") {
                    linking_flags.push(flag);
                } else if flag == "--extern" || flag == "-L" {
                    let arg = next(&flag, &mut iter)?;
                    linking_flags.extend([flag, arg.trim_matches('\'').to_owned()]);
                }
            }

            Ok(linking_flags)
        })
        .map(Vec::as_slice)
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
            Ok((*RE).captures(&line).and_then(|captures| {
                let args = captures[1]
                    .split(' ')
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                // Cargo prints `<wrapper> <rustc> ...` when a compiler wrapper
                // is configured. This parser only recovers flags, so locate
                // the rustc executable anywhere in the logged command.
                if args.iter().any(is_rustc)
                    && args
                        .as_slice()
                        .windows(2)
                        .any(|window| window == ["--crate-name", &snake_case(&target.name)])
                {
                    Some(args)
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
