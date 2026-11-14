//! Preserve rustc argument boundaries across `compiletest`'s string flags.
//!
//! `compiletest_rs` 0.11.2 accepts extra compiler flags only as the single
//! `Config::target_rustcflags` string and splits that string on literal spaces
//! in `runtest.rs::split_maybe_args`; it does not interpret quoting. A
//! dependency path containing a space therefore loses its argument boundary
//! before rustc runs. Whitaker rewrites only the arguments that contain a
//! literal space as rustc response files (`@path`), which rustc expands to one
//! argument per line. Everything else is joined exactly as before.

use std::{env::current_dir, ffi::OsStr, fs::write};

use anyhow::{Context, Result, anyhow, ensure};
use tempfile::{Builder, TempDir};

/// Prefix for the response-file directory. It must contain no space so that
/// the `@path` token survives `compiletest`'s split.
const DIRECTORY_PREFIX: &str = "dylint-argfile-";

/// Compiler flags joined for `compiletest`, plus the response files they name.
#[derive(Debug)]
pub(crate) struct EncodedFlags {
    /// The value for `Config::target_rustcflags`.
    pub(crate) text: String,

    /// Owns the response files until `compiletest` has run rustc. `None` when
    /// every argument stayed inline, so no directory is created.
    pub(crate) owner: Option<TempDir>,
}

/// Encode `flags` so that `compiletest`'s space split preserves boundaries.
///
/// Arguments without a literal space are joined unchanged, matching the
/// previous `join(" ")` output exactly. Each remaining argument is written
/// verbatim to its own response file and replaced at its original position by
/// an `@file` token.
///
/// # Errors
///
/// Returns an error if the working directory cannot be read, the response-file
/// directory or one of its files cannot be created, or a transported argument
/// contains a line feed, ends with a carriage return, or starts with `@`.
pub(crate) fn encode(flags: &[String]) -> Result<EncodedFlags> {
    if !flags.iter().any(|flag| needs_transport(flag)) {
        return Ok(EncodedFlags {
            text: flags.join(" "),
            owner: None,
        });
    }

    let directory = create_response_directory()?;
    let directory_name = directory
        .path()
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("Response-file directory has no usable name"))?
        .to_owned();

    let mut encoded = Vec::with_capacity(flags.len());
    for (index, flag) in flags.iter().enumerate() {
        if needs_transport(flag) {
            encoded.push(write_response_file(
                &directory,
                &directory_name,
                index,
                flag,
            )?);
        } else {
            encoded.push(flag.clone());
        }
    }

    Ok(EncodedFlags {
        text: encoded.join(" "),
        owner: Some(directory),
    })
}

/// Report whether `flag` must be transported rather than passed inline.
///
/// A whitespace-only argument stays inline because `compiletest` drops it
/// during its split; transporting it would preserve an argument that the
/// previous behaviour discarded.
fn needs_transport(flag: &str) -> bool { !flag.trim().is_empty() && flag.contains(' ') }

/// Create the space-free directory that holds the response files.
///
/// The directory is created inside the working directory, which is also the
/// working directory of the rustc process `compiletest` spawns. Its `@path`
/// tokens are therefore relative and resolve without embedding the possibly
/// spaced absolute path.
fn create_response_directory() -> Result<TempDir> {
    let working_dir = current_dir().context("Could not read the working directory")?;
    Builder::new()
        .prefix(DIRECTORY_PREFIX)
        .tempdir_in(&working_dir)
        .context("Could not create the rustc response-file directory")
}

/// Write one argument to `<index>.rsp` and return the `@` token naming it.
fn write_response_file(
    directory: &TempDir,
    directory_name: &str,
    index: usize,
    flag: &str,
) -> Result<String> {
    ensure!(
        !flag.contains('\n'),
        "Argument {index} contains a line feed: {flag:?}"
    );
    // rustc treats a `\r` before the terminating `\n` as a line ending and
    // would drop it, silently changing the argument.
    ensure!(
        !flag.ends_with('\r'),
        "Argument {index} ends with a carriage return: {flag:?}"
    );
    // rustc expands a leading `@` inside a response file as another response
    // file, which would change the argument's meaning.
    ensure!(
        !flag.starts_with('@'),
        "Argument {index} starts with `@`: {flag:?}"
    );

    let name = format!("{index}.rsp");
    let path = directory.path().join(&name);
    // rustc expands a response file into one argument per line, so one
    // trailing line feed preserves the argument verbatim.
    write(&path, format!("{flag}\n"))
        .with_context(|| format!("Could not write `{}`", path.display()))?;
    Ok(format!("@{directory_name}/{name}"))
}

#[cfg(test)]
mod tests {
    use std::{
        env::current_dir,
        fs,
        path::{Path, PathBuf},
    };

    use proptest::prelude::*;
    use rstest::rstest;

    use super::encode;

    /// Join flags without creating a response file when no argument is spaced.
    #[test]
    fn leaves_unspaced_flags_unchanged() {
        let flags = [
            "--edition=2021",
            "--extern",
            "dependency=target/debug/deps/libdependency.rlib",
            "-L",
            "dependency=target/debug/deps",
        ]
        .map(str::to_owned);

        let encoded = encode(&flags).expect("unspaced flags should encode");

        assert_eq!(encoded.text, flags.join(" "));
        assert!(
            encoded.owner.is_none(),
            "unspaced flags must not create a response directory"
        );
    }

    /// Keep whitespace-only arguments inline so `compiletest` still drops them.
    #[test]
    fn leaves_whitespace_only_arguments_inline() {
        let flags = [" ", "  ", "---"].map(str::to_owned);

        let encoded = encode(&flags).expect("whitespace-only flags should encode");

        assert_eq!(encoded.text, flags.join(" "));
        assert!(encoded.owner.is_none());
    }

    /// Replace spaced arguments in place and preserve their neighbours' order.
    #[test]
    fn replaces_spaced_arguments_in_place() {
        let spaced_extern = "dependency=/tmp/dylint testing/deps/libdependency.rlib";
        let spaced_search = "dependency=/tmp/dylint testing/deps";
        let flags = [
            "--edition=2021",
            "--extern",
            spaced_extern,
            "-L",
            spaced_search,
            "--emit=metadata",
        ]
        .map(str::to_owned);

        let encoded = encode(&flags).expect("spaced flags should encode");
        let tokens = encoded.text.split(' ').collect::<Vec<_>>();

        assert_eq!(tokens[0], "--edition=2021");
        assert_eq!(tokens[1], "--extern");
        assert_eq!(tokens[3], "-L");
        assert_eq!(tokens[5], "--emit=metadata");
        assert!(is_response_token(tokens[2]));
        assert!(is_response_token(tokens[4]));
        assert!(!tokens[2].contains(' '));
        assert!(!tokens[4].contains(' '));

        // The token resolves against the working directory while the owner is
        // alive, and its file holds exactly the original argument.
        assert_eq!(read_response(tokens[2]), spaced_extern);
        assert_eq!(read_response(tokens[4]), spaced_search);
    }

    /// Preserve backslashes in a quoted Windows-style dependency path.
    #[test]
    fn preserves_windows_backslashes() {
        let spaced = r"dependency=C:\Program Files\MSVC\lib\x64\libdependency.rlib";
        let flags = ["--extern", spaced].map(str::to_owned);

        let encoded = encode(&flags).expect("spaced Windows path should encode");
        let token = encoded
            .text
            .split(' ')
            .nth(1)
            .expect("encoded text should contain the response token");

        assert_eq!(read_response(token), spaced);
    }

    /// Leave an existing top-level response-file argument untouched.
    #[test]
    fn leaves_inline_response_file_arguments_unchanged() {
        let flags = ["@existing", "--extern", "dependency=/a b/lib.rlib"].map(str::to_owned);

        let encoded = encode(&flags).expect("inline response file should encode");
        let tokens = encoded.text.split(' ').collect::<Vec<_>>();

        assert_eq!(tokens[0], "@existing");
        assert!(is_response_token(tokens[2]));
    }

    /// Remove the response-file directory when the owner is dropped.
    #[test]
    fn removes_response_directory_after_owner_drops() {
        let flags = ["dependency=/tmp/dylint testing/deps"].map(str::to_owned);

        let encoded = encode(&flags).expect("spaced flag should encode");
        let token = encoded
            .text
            .split(' ')
            .next()
            .expect("encoded text should contain the response token");
        let directory = response_path(token)
            .parent()
            .expect("response file should have a parent")
            .to_path_buf();
        assert!(directory.exists());

        drop(encoded);

        assert!(!directory.exists(), "response directory should be removed");
    }

    /// Reject arguments that a response file cannot transport faithfully.
    #[rstest]
    #[case::line_feed("dependency=/a b\nc", "line feed")]
    #[case::trailing_carriage_return("dependency=/a b\r", "carriage return")]
    #[case::leading_at("@dependency=/a b", "starts with `@`")]
    fn rejects_untransportable_arguments(#[case] flag: &str, #[case] expected: &str) {
        let error = encode(&[flag.to_owned()]).expect_err("argument should be rejected");

        assert!(
            error.to_string().contains(expected),
            "unexpected error: {error}"
        );
    }

    proptest! {
        /// Expanding the encoded text reproduces the transportable arguments.
        #[test]
        fn encoded_flags_expand_to_the_original_arguments(
            flags in prop::collection::vec(transportable_argument(), 0..6)
        ) {
            let encoded = encode(&flags).expect("transportable flags should encode");
            let expanded = encoded
                .text
                .split(' ')
                .filter(|token| !token.trim().is_empty())
                .map(|token| {
                    if is_response_token(token) {
                        read_response(token)
                    } else {
                        token.to_owned()
                    }
                })
                .collect::<Vec<_>>();
            let expected = flags
                .iter()
                .filter(|flag| !flag.trim().is_empty())
                .cloned()
                .collect::<Vec<_>>();

            prop_assert_eq!(expanded, expected);
        }

        /// Arguments without a space are joined and need no response directory.
        #[test]
        fn unspaced_flags_are_joined_without_a_directory(
            flags in prop::collection::vec(unspaced_argument(), 0..6)
        ) {
            let encoded = encode(&flags).expect("unspaced flags should encode");

            prop_assert_eq!(&encoded.text, &flags.join(" "));
            prop_assert!(encoded.owner.is_none());
        }

        /// Arguments a response file cannot represent faithfully are rejected.
        #[test]
        fn rejects_arguments_that_would_change_meaning(
            // A whitespace-only argument stays inline by design, so it can
            // never reach the rejection checks below.
            argument in transportable_argument().prop_filter(
                "the argument must need transport",
                |argument| argument.contains(' ') && !argument.trim().is_empty()
            )
        ) {
            for rejected in [
                format!("{argument}\n"),
                format!("{argument}\r"),
                format!("@{argument}"),
            ] {
                prop_assert!(
                    encode(std::slice::from_ref(&rejected)).is_err(),
                    "accepted {rejected:?}"
                );
            }
        }
    }

    /// Generate one fragment of an argument.
    fn atom() -> impl Strategy<Value = String> {
        prop::sample::select(vec![
            "a".to_owned(),
            " ".to_owned(),
            "\t".to_owned(),
            "=".to_owned(),
            "@".to_owned(),
            "\r".to_owned(),
            "\u{e9}".to_owned(),
            "\u{1f600}".to_owned(),
        ])
    }

    /// Generate an argument a response file can transport faithfully.
    fn transportable_argument() -> impl Strategy<Value = String> {
        prop::collection::vec(atom(), 0..6)
            .prop_map(|parts| parts.concat())
            .prop_filter("response files cannot represent it", |argument: &String| {
                !argument.contains('\n') && !argument.ends_with('\r') && !argument.starts_with('@')
            })
    }

    /// Generate an argument that stays inline because it contains no space.
    fn unspaced_argument() -> impl Strategy<Value = String> {
        prop::collection::vec(atom(), 0..6)
            .prop_map(|parts| parts.concat())
            .prop_filter(
                "the argument must not contain a space",
                |argument: &String| !argument.contains(' '),
            )
    }

    /// Report whether `token` is an `@` reference to a response file.
    fn is_response_token(token: &str) -> bool {
        token.starts_with('@')
            && Path::new(token)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rsp"))
    }

    /// Return the resolved path of a response token.
    fn response_path(token: &str) -> PathBuf {
        current_dir()
            .expect("working directory should be readable")
            .join(token.trim_start_matches('@'))
    }

    /// Read a response file and remove its single terminating line feed.
    fn read_response(token: &str) -> String {
        let contents =
            fs::read_to_string(response_path(token)).expect("response file should exist");
        contents
            .strip_suffix('\n')
            .expect("response file should end with one line feed")
            .to_owned()
    }
}
