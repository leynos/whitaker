//! Tests for recovering example linking flags from verbose Cargo output.

use std::path::Path;

use rstest::rstest;

use super::{ExampleName, linking_flags_from_build_log};

const RUSTC: &str = "/toolchains/nightly/bin/rustc";
const ARGUMENTS: &str = concat!(
    "--crate-name demo --edition=2024 examples/demo.rs --crate-type bin ",
    "-L dependency=/target/debug/deps ",
    "--extern 'rstest=/target/debug/deps/librstest-1.rlib' -C prefer-dynamic",
);

fn demo() -> ExampleName { ExampleName::new("demo") }

fn running(prefix: &str) -> String { format!("     Running `{prefix}{RUSTC} {ARGUMENTS}`") }

fn expected_flags() -> Vec<String> {
    [
        "--edition=2024",
        "-L",
        "dependency=/target/debug/deps",
        "--extern",
        "rstest=/target/debug/deps/librstest-1.rlib",
    ]
    .map(String::from)
    .to_vec()
}

#[rstest]
#[case::bare_compiler("")]
#[case::ci_cache_wrapper("sccache ")]
#[case::admission_wrapper("/home/user/.local/bin/build-limits-rustc ")]
#[case::wrapper_chain("/usr/bin/sccache /usr/local/bin/build-limits-rustc ")]
#[case::environment_assignments("CARGO=/usr/bin/cargo CARGO_PKG_NAME=demo ")]
#[case::compiler_named_in_environment("RUSTC=/toolchains/nightly/bin/rustc ")]
fn recovers_flags_past_any_prefix(#[case] prefix: &str) {
    let log = format!(
        "   Compiling demo v0.1.0\n{}\n    Finished",
        running(prefix)
    );

    let flags = linking_flags_from_build_log(&log, &demo()).expect("one invocation should match");

    assert_eq!(flags, expected_flags());
}

// CI forces Cargo to style its status words with terminal escapes.
#[test]
fn recovers_flags_from_styled_output() {
    let styled = running("sccache ").replacen("Running", "\u{1b}[1m\u{1b}[32mRunning\u{1b}[0m", 1);

    let flags = linking_flags_from_build_log(&styled, &demo()).expect("styling should be ignored");

    assert_eq!(flags, expected_flags());
}

#[test]
fn ignores_invocations_for_other_crates() {
    let other = running("sccache ").replace("--crate-name demo", "--crate-name helper");
    let log = format!("{other}\n{}", running("sccache "));

    let flags = linking_flags_from_build_log(&log, &demo()).expect("only `demo` should match");

    assert_eq!(flags, expected_flags());
}

#[rstest]
#[case::no_running_lines("   Compiling demo v0.1.0\n    Finished", "found no")]
#[case::wrapper_without_compiler("     Running `sccache --crate-name demo`", "found no")]
#[case::build_script(
    "     Running `/target/debug/build/demo-1/build-script-build`",
    "found no"
)]
fn reports_missing_invocations(#[case] log: &str, #[case] expected: &str) {
    let error = linking_flags_from_build_log(log, &demo()).expect_err("nothing should match");

    assert!(error.contains(expected), "{error}");
}

#[test]
fn reports_ambiguous_invocations() {
    let log = format!("{}\n{}", running(""), running("sccache "));

    let error = linking_flags_from_build_log(&log, &demo()).expect_err("two invocations match");

    assert!(error.contains("several"), "{error}");
}

#[test]
fn reports_flags_missing_their_values() {
    let log = format!("     Running `{RUSTC} --crate-name demo --extern`");

    let error = linking_flags_from_build_log(&log, &demo()).expect_err("`--extern` lacks a value");

    assert!(error.contains("--extern"), "{error}");
}

#[rstest]
#[case::binary(format!("demo{}", std::env::consts::EXE_SUFFIX), true)]
#[case::hashed_artefact(String::from("demo-0123abcd"), true)]
#[case::hashed_debug_info(String::from("demo-0123abcd.d"), true)]
#[case::longer_example_name(String::from("demo_extended"), false)]
#[case::other_example(String::from("other-0123abcd"), false)]
fn recognizes_built_example_artefacts(#[case] file_name: String, #[case] is_match: bool) {
    assert_eq!(
        demo().is_built_artefact(&Path::new("/target/debug/examples").join(file_name)),
        is_match,
    );
}
